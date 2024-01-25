use std::borrow::Cow;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_channel::{Receiver, Sender};
use async_ssh2_lite::ssh2::ErrorCode;
use async_ssh2_lite::{AsyncChannel, AsyncSession, Error};
use async_trait::async_trait;
use futures::io::BufReader;
use futures::{AsyncBufReadExt, AsyncWriteExt, StreamExt};
use hydroflow_cli_integration::ServerBindConfig;
use nanoid::nanoid;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

use super::progress::ProgressTracker;
use super::util::async_retry;
use super::{LaunchedBinary, LaunchedHost, ResourceResult, ServerStrategy};
use crate::util::prioritized_broadcast;

struct LaunchedSSHBinary {
    _resource_result: Arc<ResourceResult>,
    session: Option<AsyncSession<TcpStream>>,
    channel: AsyncChannel<TcpStream>,
    stdin_sender: Sender<String>,
    stdout_receivers: Arc<RwLock<Vec<Sender<String>>>>,
    stdout_cli_receivers: Arc<RwLock<Option<tokio::sync::oneshot::Sender<String>>>>,
    stderr_receivers: Arc<RwLock<Vec<Sender<String>>>>,
}

#[async_trait]
impl LaunchedBinary for LaunchedSSHBinary {
    async fn stdin(&self) -> Sender<String> {
        self.stdin_sender.clone()
    }

    async fn cli_stdout(&self) -> tokio::sync::oneshot::Receiver<String> {
        let mut receivers = self.stdout_cli_receivers.write().await;

        if receivers.is_some() {
            panic!("Only one CLI stdout receiver is allowed at a time");
        }

        let (sender, receiver) = tokio::sync::oneshot::channel::<String>();
        *receivers = Some(sender);
        receiver
    }

    async fn stdout(&self) -> Receiver<String> {
        let mut receivers = self.stdout_receivers.write().await;
        let (sender, receiver) = async_channel::unbounded::<String>();
        receivers.push(sender);
        receiver
    }

    async fn stderr(&self) -> Receiver<String> {
        let mut receivers = self.stderr_receivers.write().await;
        let (sender, receiver) = async_channel::unbounded::<String>();
        receivers.push(sender);
        receiver
    }

    async fn exit_code(&self) -> Option<i32> {
        // until the program exits, the exit status is meaningless
        if self.channel.eof() {
            self.channel.exit_status().ok()
        } else {
            None
        }
    }

    async fn wait(&mut self) -> Option<i32> {
        self.channel.wait_eof().await.unwrap();
        let ret = self.exit_code().await;
        self.channel.wait_close().await.unwrap();
        ret
    }
}

impl Drop for LaunchedSSHBinary {
    fn drop(&mut self) {
        let session = self.session.take().unwrap();
        std::thread::scope(|s| {
            s.spawn(|| {
                let runtime = tokio::runtime::Builder::new_multi_thread().build().unwrap();
                runtime
                    .block_on(session.disconnect(None, "", None))
                    .unwrap();
            })
            .join()
            .unwrap();
        });
    }
}

#[async_trait]
pub trait LaunchedSSHHost: Send + Sync {
    fn server_config(&self, bind_type: &ServerStrategy) -> ServerBindConfig;
    fn resource_result(&self) -> &Arc<ResourceResult>;
    fn ssh_user(&self) -> &str;
    async fn open_ssh_session(&self) -> Result<AsyncSession<TcpStream>>;
}

#[async_trait]
impl<T: LaunchedSSHHost> LaunchedHost for T {
    fn server_config(&self, bind_type: &ServerStrategy) -> ServerBindConfig {
        LaunchedSSHHost::server_config(self, bind_type)
    }

    async fn copy_binary(&self, binary: Arc<(String, Vec<u8>, PathBuf)>) -> Result<()> {
        let session = self.open_ssh_session().await?;

        let sftp = async_retry(
            &|| async { Ok(session.sftp().await?) },
            10,
            Duration::from_secs(1),
        )
        .await?;

        // we may be deploying multiple binaries, so give each a unique name
        let unique_name = &binary.0;

        let user = self.ssh_user();
        let binary_path = PathBuf::from(format!("/home/{user}/hydro-{unique_name}"));

        if sftp.stat(&binary_path).await.is_err() {
            let random = nanoid!(8);
            let temp_path = PathBuf::from(format!("/home/{user}/hydro-{random}"));
            let sftp = &sftp;

            ProgressTracker::rich_leaf(
                format!("uploading binary to /home/{user}/hydro-{unique_name}"),
                |set_progress, _| {
                    let binary = &binary;
                    let binary_path = &binary_path;
                    async move {
                        let mut created_file = sftp.create(&temp_path).await?;

                        let mut index = 0;
                        while index < binary.1.len() {
                            let written = created_file
                                .write(
                                    &binary.1
                                        [index..std::cmp::min(index + 128 * 1024, binary.1.len())],
                                )
                                .await?;
                            index += written;
                            set_progress(((index as f64 / binary.1.len() as f64) * 100.0) as u64);
                        }
                        let mut orig_file_stat = sftp.stat(&temp_path).await?;
                        orig_file_stat.perm = Some(0o755); // allow the copied binary to be executed by anyone
                        created_file.setstat(orig_file_stat).await?;
                        created_file.close().await?;
                        drop(created_file);

                        match sftp.rename(&temp_path, binary_path, None).await {
                            Ok(_) => {}
                            Err(Error::Ssh2(e)) if e.code() == ErrorCode::SFTP(4) => {
                                // file already exists
                                sftp.unlink(&temp_path).await?;
                            }
                            Err(e) => return Err(e.into()),
                        }

                        anyhow::Ok(())
                    }
                },
            )
            .await?;
        }
        drop(sftp);

        Ok(())
    }

    async fn launch_binary(
        &self,
        id: String,
        binary: Arc<(String, Vec<u8>, PathBuf)>,
        args: &[String],
    ) -> Result<Arc<RwLock<dyn LaunchedBinary>>> {
        let session = self.open_ssh_session().await?;

        let unique_name = &binary.0;

        let user = self.ssh_user();
        let binary_path = PathBuf::from(format!("/home/{user}/hydro-{unique_name}"));

        let channel = ProgressTracker::leaf(
            format!("launching binary /home/{user}/hydro-{unique_name}"),
            async {
                let mut channel =
                    async_retry(
                        &|| async {
                            Ok(tokio::time::timeout(
                                Duration::from_secs(60),
                                session.channel_session(),
                            )
                            .await??)
                        },
                        10,
                        Duration::from_secs(1),
                    )
                    .await?;
                let binary_path_string = binary_path.to_str().unwrap();
                let args_string = args
                    .iter()
                    .map(|s| shell_escape::unix::escape(Cow::from(s)))
                    .fold("".to_string(), |acc, v| format!("{acc} {v}"));
                channel
                    .exec(&format!("{binary_path_string}{args_string}"))
                    .await?;

                anyhow::Ok(channel)
            },
        )
        .await?;

        let (stdin_sender, mut stdin_receiver) = async_channel::unbounded::<String>();
        let mut stdin = channel.stream(0); // stream 0 is stdout/stdin, we use it for stdin
        tokio::spawn(async move {
            while let Some(line) = stdin_receiver.next().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }

                stdin.flush().await.unwrap();
            }
        });

        let id_clone = id.clone();
        let (stdout_cli_receivers, stdout_receivers) =
            prioritized_broadcast(BufReader::new(channel.stream(0)).lines(), move |s| {
                println!("[{id_clone}] {s}")
            });
        let (_, stderr_receivers) =
            prioritized_broadcast(BufReader::new(channel.stderr()).lines(), move |s| {
                eprintln!("[{id}] {s}")
            });

        ProgressTracker::println("[launch_binary] Finished launching binary");

        Ok(Arc::new(RwLock::new(LaunchedSSHBinary {
            _resource_result: self.resource_result().clone(),
            session: Some(session),
            channel,
            stdin_sender,
            stdout_cli_receivers,
            stdout_receivers,
            stderr_receivers,
        })))
    }

    async fn forward_port(&self, addr: &SocketAddr) -> Result<SocketAddr> {
        ProgressTracker::println("Forwarding port");
        let session = self.open_ssh_session().await?;

        ProgressTracker::println("[forward_port]: binding to tcp listener localhost");
        let local_port = TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = local_port.local_addr()?;
        ProgressTracker::println("[forward_port]: bound to localhost");

        let internal_ip = addr.ip().to_string();
        let port = addr.port();

        tokio::spawn(async move {
            #[allow(clippy::never_loop)]
            while let Ok((mut local_stream, _)) = local_port.accept().await {
                let mut channel = session
                    .channel_direct_tcpip(&internal_ip, port, None)
                    .await
                    .unwrap();
                let _ = tokio::io::copy_bidirectional(&mut local_stream, &mut channel).await;
                break;
                // TODO(shadaj): we should be returning an Arc so that we know
                // if anyone wants to connect to this forwarded port
            }

            ProgressTracker::println("[hydro] closing forwarded port");
        });

        Ok(local_addr)
    }
}
