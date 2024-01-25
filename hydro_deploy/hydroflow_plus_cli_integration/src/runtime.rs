use std::cell::RefCell;
use std::rc::Rc;

use hydroflow_plus::lang::parse::Pipeline;
use hydroflow_plus::location::{
    Cluster, ClusterSpec, Deploy, HfSendManyToMany, HfSendManyToOne, HfSendOneToMany,
    HfSendOneToOne, Location, ProcessSpec,
};
use hydroflow_plus::util::cli::{
    ConnectedDemux, ConnectedDirect, ConnectedSink, ConnectedSource, ConnectedTagged, HydroCLI,
};
use hydroflow_plus::FlowBuilder;
use stageleft::{q, Quoted, RuntimeData};
use syn::parse_quote;

use super::HydroflowPlusMeta;

pub struct CLIRuntime {}

impl<'a> Deploy<'a> for CLIRuntime {
    type Process = CLIRuntimeNode<'a>;
    type Cluster = CLIRuntimeCluster<'a>;
    type Meta = ();
    type RuntimeID = usize;
    type ProcessPort = String;
    type ClusterPort = String;
}

#[derive(Clone)]
pub struct CLIRuntimeNode<'a> {
    id: usize,
    builder: &'a FlowBuilder<'a, CLIRuntime>,
    next_port: Rc<RefCell<usize>>,
    cli: RuntimeData<&'a HydroCLI<HydroflowPlusMeta>>,
}

impl<'a> Location<'a> for CLIRuntimeNode<'a> {
    type Port = String;
    type Meta = ();

    fn id(&self) -> usize {
        self.id
    }

    fn flow_builder(&self) -> (&'a RefCell<usize>, &'a hydroflow_plus::builder::Builders) {
        self.builder.builder_components()
    }

    fn next_port(&self) -> String {
        let next_send_port = *self.next_port.borrow();
        *self.next_port.borrow_mut() += 1;
        format!("port_{}", next_send_port)
    }

    fn update_meta(&mut self, _meta: &Self::Meta) {}
}

#[derive(Clone)]
pub struct CLIRuntimeCluster<'a> {
    id: usize,
    builder: &'a FlowBuilder<'a, CLIRuntime>,
    next_port: Rc<RefCell<usize>>,
    cli: RuntimeData<&'a HydroCLI<HydroflowPlusMeta>>,
}

impl<'a> Location<'a> for CLIRuntimeCluster<'a> {
    type Port = String;
    type Meta = ();

    fn id(&self) -> usize {
        self.id
    }

    fn flow_builder(&self) -> (&'a RefCell<usize>, &'a hydroflow_plus::builder::Builders) {
        self.builder.builder_components()
    }

    fn next_port(&self) -> String {
        let next_send_port = *self.next_port.borrow();
        *self.next_port.borrow_mut() += 1;
        format!("port_{}", next_send_port)
    }

    fn update_meta(&mut self, _meta: &Self::Meta) {}
}

impl<'a> Cluster<'a> for CLIRuntimeCluster<'a> {
    fn ids(&self) -> impl Quoted<'a, &'a Vec<u32>> + Copy + 'a {
        let cli = self.cli;
        let self_id = self.id;
        q!(cli.meta.clusters.get(&self_id).unwrap())
    }
}

impl<'a> HfSendOneToOne<'a, CLIRuntimeNode<'a>> for CLIRuntimeNode<'a> {
    fn connect(&self, _other: &CLIRuntimeNode, _source_port: &String, _recipient_port: &String) {}

    fn gen_sink_statement(&self, port: &String) -> Pipeline {
        let self_cli = self.cli;
        let port = port.as_str();
        let sink_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedDirect>()
                .into_sink()
        })
        .splice();

        parse_quote!(dest_sink(#sink_quote))
    }

    fn gen_source_statement(other: &CLIRuntimeNode<'a>, port: &String) -> Pipeline {
        let self_cli = other.cli;
        let port = port.as_str();
        let source_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedDirect>()
                .into_source()
        })
        .splice();

        parse_quote!(source_stream(#source_quote))
    }
}

impl<'a> HfSendManyToOne<'a, CLIRuntimeNode<'a>> for CLIRuntimeCluster<'a> {
    fn connect(&self, _other: &CLIRuntimeNode, _source_port: &String, _recipient_port: &String) {}

    fn gen_sink_statement(&self, port: &String) -> Pipeline {
        let self_cli = self.cli;
        let port = port.as_str();
        let sink_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedDirect>()
                .into_sink()
        })
        .splice();

        parse_quote!(dest_sink(#sink_quote))
    }

    fn gen_source_statement(other: &CLIRuntimeNode<'a>, port: &String) -> Pipeline {
        let self_cli = other.cli;
        let port = port.as_str();
        let source_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedTagged<ConnectedDirect>>()
                .into_source()
        })
        .splice();

        parse_quote!(source_stream(#source_quote))
    }
}

impl<'a> HfSendOneToMany<'a, CLIRuntimeCluster<'a>> for CLIRuntimeNode<'a> {
    fn connect(&self, _other: &CLIRuntimeCluster, _source_port: &String, _recipient_port: &String) {
    }

    fn gen_sink_statement(&self, port: &String) -> Pipeline {
        let self_cli = self.cli;
        let port = port.as_str();

        let sink_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedDemux<ConnectedDirect>>()
                .into_sink()
        })
        .splice();

        parse_quote!(dest_sink(#sink_quote))
    }

    fn gen_source_statement(other: &CLIRuntimeCluster<'a>, port: &String) -> Pipeline {
        let self_cli = other.cli;
        let port = port.as_str();

        let source_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedDirect>()
                .into_source()
        })
        .splice();

        parse_quote!(source_stream(#source_quote))
    }
}

impl<'a> HfSendManyToMany<'a, CLIRuntimeCluster<'a>> for CLIRuntimeCluster<'a> {
    fn connect(&self, _other: &CLIRuntimeCluster, _source_port: &String, _recipient_port: &String) {
    }

    fn gen_sink_statement(&self, port: &String) -> Pipeline {
        let self_cli = self.cli;
        let port = port.as_str();

        let sink_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedDemux<ConnectedDirect>>()
                .into_sink()
        })
        .splice();

        parse_quote!(dest_sink(#sink_quote))
    }

    fn gen_source_statement(other: &CLIRuntimeCluster<'a>, port: &String) -> Pipeline {
        let self_cli = other.cli;
        let port = port.as_str();

        let source_quote = q!({
            self_cli
                .port(port)
                .connect_local_blocking::<ConnectedTagged<ConnectedDirect>>()
                .into_source()
        })
        .splice();

        parse_quote!(source_stream(#source_quote))
    }
}

impl<'cli> ProcessSpec<'cli, CLIRuntime> for RuntimeData<&'cli HydroCLI<HydroflowPlusMeta>> {
    fn build(
        &self,
        id: usize,
        builder: &'cli FlowBuilder<'cli, CLIRuntime>,
        _meta: &mut (),
    ) -> CLIRuntimeNode<'cli> {
        CLIRuntimeNode {
            id,
            builder,
            next_port: Rc::new(RefCell::new(0)),
            cli: *self,
        }
    }
}

impl<'cli> ClusterSpec<'cli, CLIRuntime> for RuntimeData<&'cli HydroCLI<HydroflowPlusMeta>> {
    fn build(
        &self,
        id: usize,
        builder: &'cli FlowBuilder<'cli, CLIRuntime>,
        _meta: &mut (),
    ) -> CLIRuntimeCluster<'cli> {
        CLIRuntimeCluster {
            id,
            builder,
            next_port: Rc::new(RefCell::new(0)),
            cli: *self,
        }
    }
}
