use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use async_channel::Receiver;
use hydro_deploy::custom_service::CustomClientPort;
use hydro_deploy::hydroflow_crate::ports::{
    DemuxSink, HydroflowSink, HydroflowSource, TaggedSource,
};
use hydro_deploy::hydroflow_crate::HydroflowCrateService;
use hydro_deploy::{Deployment, Host};
use hydroflow_plus::builder::Builders;
use hydroflow_plus::location::{
    Cluster, ClusterSpec, Deploy, HfSendManyToMany, HfSendManyToOne, HfSendOneToMany,
    HfSendOneToOne, Location, ProcessSpec,
};
use hydroflow_plus::FlowBuilder;
use stageleft::internal::syn::parse_quote;
use stageleft::q;
use tokio::sync::RwLock;

use super::HydroflowPlusMeta;

pub struct HydroDeploy {}

impl<'a> Deploy<'a> for HydroDeploy {
    type Process = DeployNode<'a>;
    type Cluster = DeployCluster<'a>;
    type Meta = HashMap<usize, Vec<u32>>;
    type RuntimeID = ();
    type ProcessPort = DeployPort<DeployNode<'a>>;
    type ClusterPort = DeployPort<DeployCluster<'a>>;
}

pub trait DeployCrateWrapper {
    fn underlying(&self) -> Arc<RwLock<HydroflowCrateService>>;

    #[allow(async_fn_in_trait)]
    async fn create_sender(
        &self,
        port: &str,
        deployment: &mut Deployment,
        on: &Arc<RwLock<impl Host + 'static>>,
    ) -> CustomClientPort {
        let sender_service = deployment.CustomService(on.clone(), vec![]);
        let mut sender_port = sender_service.read().await.declare_client(&sender_service);
        let mut recipient = self
            .underlying()
            .read()
            .await
            .get_port(port.to_string(), &self.underlying());

        sender_port.send_to(&mut recipient);
        sender_port
    }

    #[allow(async_fn_in_trait)]
    async fn stdout(&self) -> Receiver<String> {
        self.underlying().read().await.stdout().await
    }

    #[allow(async_fn_in_trait)]
    async fn stderr(&self) -> Receiver<String> {
        self.underlying().read().await.stderr().await
    }
}

#[derive(Clone)]
pub struct DeployNode<'a> {
    id: usize,
    builder: &'a FlowBuilder<'a, HydroDeploy>,
    next_port: Rc<RefCell<usize>>,
    underlying: Arc<RwLock<HydroflowCrateService>>,
}

impl<'a> DeployCrateWrapper for DeployNode<'a> {
    fn underlying(&self) -> Arc<RwLock<HydroflowCrateService>> {
        self.underlying.clone()
    }
}

pub struct DeployPort<N> {
    node: N,
    port: String,
}

impl<'a> DeployPort<DeployNode<'a>> {
    pub async fn create_sender(
        &self,
        deployment: &mut Deployment,
        on: &Arc<RwLock<impl Host + 'static>>,
    ) -> CustomClientPort {
        self.node.create_sender(&self.port, deployment, on).await
    }
}

impl<'a> DeployPort<DeployCluster<'a>> {
    pub async fn create_senders(
        &self,
        deployment: &mut Deployment,
        on: &Arc<RwLock<impl Host + 'static>>,
    ) -> Vec<CustomClientPort> {
        let mut out = vec![];
        for member in &self.node.members {
            out.push(member.create_sender(&self.port, deployment, on).await);
        }

        out
    }
}

impl<'a> Location<'a> for DeployNode<'a> {
    type Port = DeployPort<Self>;
    type Meta = HashMap<usize, Vec<u32>>;

    fn id(&self) -> usize {
        self.id
    }

    fn flow_builder(&self) -> (&'a RefCell<usize>, &'a Builders) {
        self.builder.builder_components()
    }

    fn next_port(&self) -> DeployPort<Self> {
        let next_port = *self.next_port.borrow();
        *self.next_port.borrow_mut() += 1;

        DeployPort {
            node: self.clone(),
            port: format!("port_{}", next_port),
        }
    }

    fn update_meta(&mut self, meta: &Self::Meta) {
        let mut n = self.underlying.try_write().unwrap();
        n.update_meta(HydroflowPlusMeta {
            clusters: meta.clone(),
            subgraph_id: self.id,
        });
    }
}

#[derive(Clone)]
pub struct DeployClusterNode {
    underlying: Arc<RwLock<HydroflowCrateService>>,
}

impl DeployCrateWrapper for DeployClusterNode {
    fn underlying(&self) -> Arc<RwLock<HydroflowCrateService>> {
        self.underlying.clone()
    }
}

#[derive(Clone)]
pub struct DeployCluster<'a> {
    id: usize,
    builder: &'a FlowBuilder<'a, HydroDeploy>,
    next_port: Rc<RefCell<usize>>,
    pub members: Vec<DeployClusterNode>,
}

impl<'a> Location<'a> for DeployCluster<'a> {
    type Port = DeployPort<Self>;
    type Meta = HashMap<usize, Vec<u32>>;

    fn id(&self) -> usize {
        self.id
    }

    fn flow_builder(&self) -> (&'a RefCell<usize>, &'a Builders) {
        self.builder.builder_components()
    }

    fn next_port(&self) -> DeployPort<Self> {
        let next_port = *self.next_port.borrow();
        *self.next_port.borrow_mut() += 1;

        DeployPort {
            node: self.clone(),
            port: format!("port_{}", next_port),
        }
    }

    fn update_meta(&mut self, meta: &Self::Meta) {
        let meta = HydroflowPlusMeta {
            clusters: meta.clone(),
            subgraph_id: self.id,
        };

        self.members.iter().for_each(|n| {
            let mut n = n.underlying.try_write().unwrap();
            n.update_meta(&meta);
        });
    }
}

impl<'a> Cluster<'a> for DeployCluster<'a> {
    fn ids(&self) -> impl stageleft::Quoted<'a, &'a Vec<u32>> + Copy + 'a {
        q!(panic!())
    }
}

impl<'a> HfSendOneToOne<'a, DeployNode<'a>> for DeployNode<'a> {
    fn connect(
        &self,
        other: &DeployNode<'a>,
        source_port: &DeployPort<DeployNode<'a>>,
        recipient_port: &DeployPort<DeployNode<'a>>,
    ) {
        let mut source_port = self
            .underlying
            .try_read()
            .unwrap()
            .get_port(source_port.port.clone(), &self.underlying);

        let mut recipient_port = other
            .underlying
            .try_read()
            .unwrap()
            .get_port(recipient_port.port.clone(), &other.underlying);

        source_port.send_to(&mut recipient_port);
    }

    fn gen_sink_statement(&self, _port: &Self::Port) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }

    fn gen_source_statement(
        _other: &DeployNode<'a>,
        _port: &Self::Port,
    ) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }
}

impl<'a> HfSendManyToOne<'a, DeployNode<'a>> for DeployCluster<'a> {
    fn connect(
        &self,
        other: &DeployNode<'a>,
        source_port: &DeployPort<DeployCluster<'a>>,
        recipient_port: &DeployPort<DeployNode<'a>>,
    ) {
        let mut recipient_port = other
            .underlying
            .try_read()
            .unwrap()
            .get_port(recipient_port.port.clone(), &other.underlying)
            .merge();

        for (i, node) in self.members.iter().enumerate() {
            let source_port = node
                .underlying
                .try_read()
                .unwrap()
                .get_port(source_port.port.clone(), &node.underlying);

            TaggedSource {
                source: Arc::new(RwLock::new(source_port)),
                tag: i as u32,
            }
            .send_to(&mut recipient_port);
        }
    }

    fn gen_sink_statement(&self, _port: &Self::Port) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }

    fn gen_source_statement(
        _other: &DeployNode<'a>,
        _port: &DeployPort<DeployNode<'a>>,
    ) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }
}

impl<'a> HfSendOneToMany<'a, DeployCluster<'a>> for DeployNode<'a> {
    fn connect(
        &self,
        other: &DeployCluster<'a>,
        source_port: &DeployPort<DeployNode<'a>>,
        recipient_port: &DeployPort<DeployCluster<'a>>,
    ) {
        let mut source_port = self
            .underlying
            .try_read()
            .unwrap()
            .get_port(source_port.port.clone(), &self.underlying);

        let mut recipient_port = DemuxSink {
            demux: other
                .members
                .iter()
                .enumerate()
                .map(|(id, c)| {
                    let n = c.underlying.try_read().unwrap();
                    (
                        id as u32,
                        Arc::new(RwLock::new(
                            n.get_port(recipient_port.port.clone(), &c.underlying),
                        )) as Arc<RwLock<dyn HydroflowSink + 'static>>,
                    )
                })
                .collect(),
        };

        source_port.send_to(&mut recipient_port);
    }

    fn gen_sink_statement(&self, _port: &Self::Port) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }

    fn gen_source_statement(
        _other: &DeployCluster<'a>,
        _port: &DeployPort<DeployCluster<'a>>,
    ) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }
}

impl<'a> HfSendManyToMany<'a, DeployCluster<'a>> for DeployCluster<'a> {
    fn connect(
        &self,
        other: &DeployCluster<'a>,
        source_port: &DeployPort<DeployCluster<'a>>,
        recipient_port: &DeployPort<DeployCluster<'a>>,
    ) {
        for (i, sender) in self.members.iter().enumerate() {
            let source_port = sender
                .underlying
                .try_read()
                .unwrap()
                .get_port(source_port.port.clone(), &sender.underlying);

            let mut recipient_port = DemuxSink {
                demux: other
                    .members
                    .iter()
                    .enumerate()
                    .map(|(id, c)| {
                        let n = c.underlying.try_read().unwrap();
                        (
                            id as u32,
                            Arc::new(RwLock::new(
                                n.get_port(recipient_port.port.clone(), &c.underlying)
                                    .merge(),
                            ))
                                as Arc<RwLock<dyn HydroflowSink + 'static>>,
                        )
                    })
                    .collect(),
            };

            TaggedSource {
                source: Arc::new(RwLock::new(source_port)),
                tag: i as u32,
            }
            .send_to(&mut recipient_port);
        }
    }

    fn gen_sink_statement(&self, _port: &Self::Port) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }

    fn gen_source_statement(
        _other: &DeployCluster<'a>,
        _port: &DeployPort<DeployCluster<'a>>,
    ) -> hydroflow_plus::lang::parse::Pipeline {
        parse_quote!(null())
    }
}

type CrateBuilder<'a> = dyn FnMut() -> Arc<RwLock<HydroflowCrateService>> + 'a;

pub struct DeployProcessSpec<'a>(RefCell<Box<CrateBuilder<'a>>>);

impl<'a> DeployProcessSpec<'a> {
    pub fn new<F: FnMut() -> Arc<RwLock<HydroflowCrateService>> + 'a>(f: F) -> Self {
        Self(RefCell::new(Box::new(f)))
    }
}

impl<'a: 'b, 'b> ProcessSpec<'a, HydroDeploy> for DeployProcessSpec<'b> {
    fn build(
        &self,
        id: usize,
        builder: &'a FlowBuilder<'a, HydroDeploy>,
        _meta: &mut HashMap<usize, Vec<u32>>,
    ) -> DeployNode<'a> {
        DeployNode {
            id,
            builder,
            next_port: Rc::new(RefCell::new(0)),
            underlying: (self.0.borrow_mut())(),
        }
    }
}

type ClusterSpecFn<'a> = dyn FnMut() -> Vec<Arc<RwLock<HydroflowCrateService>>> + 'a;

pub struct DeployClusterSpec<'a>(RefCell<Box<ClusterSpecFn<'a>>>);

impl<'a> DeployClusterSpec<'a> {
    pub fn new<F: FnMut() -> Vec<Arc<RwLock<HydroflowCrateService>>> + 'a>(f: F) -> Self {
        Self(RefCell::new(Box::new(f)))
    }
}

impl<'a: 'b, 'b> ClusterSpec<'a, HydroDeploy> for DeployClusterSpec<'b> {
    fn build(
        &self,
        id: usize,
        builder: &'a FlowBuilder<'a, HydroDeploy>,
        meta: &mut HashMap<usize, Vec<u32>>,
    ) -> DeployCluster<'a> {
        let cluster_nodes = (self.0.borrow_mut())();
        meta.insert(id, (0..(cluster_nodes.len() as u32)).collect());

        DeployCluster {
            id,
            builder,
            next_port: Rc::new(RefCell::new(0)),
            members: cluster_nodes
                .into_iter()
                .map(|u| DeployClusterNode { underlying: u })
                .collect(),
        }
    }
}
