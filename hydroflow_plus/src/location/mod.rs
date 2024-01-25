use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::time::Duration;

use hydroflow::bytes::BytesMut;
use hydroflow::futures::stream::Stream as FuturesStream;
use proc_macro2::Span;
use stageleft::{q, Quoted};
use syn::parse_quote;

use crate::builder::Builders;
use crate::stream::{Async, Windowed};
use crate::{FlowBuilder, HfCycle, Stream};

pub mod graphs;
pub use graphs::*;

pub mod network;
pub use network::*;

pub trait LocalDeploy<'a> {
    type Process: Location<'a, Meta = Self::Meta>;
    type Cluster: Location<'a, Meta = Self::Meta> + Cluster<'a>;
    type Meta: Default;
    type RuntimeID;
}

pub trait Deploy<'a> {
    type Process: Location<'a, Meta = Self::Meta, Port = Self::ProcessPort>
        + HfSendOneToOne<'a, Self::Process>
        + HfSendOneToMany<'a, Self::Cluster>;
    type Cluster: Location<'a, Meta = Self::Meta, Port = Self::ClusterPort>
        + HfSendManyToOne<'a, Self::Process>
        + HfSendManyToMany<'a, Self::Cluster>
        + Cluster<'a>;
    type ProcessPort;
    type ClusterPort;
    type Meta: Default;
    type RuntimeID;
}

impl<
        'a,
        T: Deploy<'a, Process = N, Cluster = C, Meta = M, RuntimeID = R>,
        N: Location<'a, Meta = M> + HfSendOneToOne<'a, N> + HfSendOneToMany<'a, C>,
        C: Location<'a, Meta = M> + HfSendManyToOne<'a, N> + HfSendManyToMany<'a, C> + Cluster<'a>,
        M: Default,
        R,
    > LocalDeploy<'a> for T
{
    type Process = N;
    type Cluster = C;
    type Meta = M;
    type RuntimeID = R;
}

pub trait ProcessSpec<'a, D: LocalDeploy<'a> + ?Sized> {
    fn build(&self, id: usize, builder: &'a FlowBuilder<'a, D>, meta: &mut D::Meta) -> D::Process;
}

pub trait ClusterSpec<'a, D: LocalDeploy<'a> + ?Sized> {
    fn build(&self, id: usize, builder: &'a FlowBuilder<'a, D>, meta: &mut D::Meta) -> D::Cluster;
}

pub trait Location<'a>: Clone {
    type Port;
    type Meta;

    fn id(&self) -> usize;
    fn flow_builder(&self) -> (&'a RefCell<usize>, &'a Builders);
    fn next_port(&self) -> Self::Port;

    fn update_meta(&mut self, meta: &Self::Meta);

    fn spin(&self) -> Stream<'a, (), Async, Self> {
        let (next_id_cell, builders) = self.flow_builder();

        let next_id = {
            let mut next_id = next_id_cell.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let ident = syn::Ident::new(&format!("stream_{}", next_id), Span::call_site());

        builders
            .borrow_mut()
            .as_mut()
            .unwrap()
            .entry(self.id())
            .or_default()
            .add_statement(parse_quote! {
                #ident = spin() -> tee();
            });

        Stream {
            ident,
            node: self.clone(),
            next_id: next_id_cell,
            builders,
            is_delta: false,
            _phantom: PhantomData,
        }
    }

    fn spin_batch(
        &self,
        batch_size: impl Quoted<'a, usize> + Copy + 'a,
    ) -> Stream<'a, (), Windowed, Self> {
        self.spin()
            .flat_map(q!(move |_| 0..batch_size))
            .map(q!(|_| ()))
            .tick_batch()
    }

    fn source_stream<T, E: FuturesStream<Item = T> + Unpin>(
        &self,
        e: impl Quoted<'a, E>,
    ) -> Stream<'a, T, Async, Self> {
        let (next_id_cell, builders) = self.flow_builder();

        let next_id = {
            let mut next_id = next_id_cell.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let ident = syn::Ident::new(&format!("stream_{}", next_id), Span::call_site());
        let e = e.splice();

        builders
            .borrow_mut()
            .as_mut()
            .unwrap()
            .entry(self.id())
            .or_default()
            .add_statement(parse_quote! {
                #ident = source_stream(#e) -> tee();
            });

        Stream {
            ident,
            node: self.clone(),
            next_id: next_id_cell,
            builders,
            is_delta: false,
            _phantom: PhantomData,
        }
    }

    fn source_external(
        &self,
    ) -> (
        Self::Port,
        Stream<'a, Result<BytesMut, io::Error>, Async, Self>,
    )
    where
        Self: HfSendOneToOne<'a, Self>,
    {
        let (next_id_cell, builders) = self.flow_builder();

        let next_id = {
            let mut next_id = next_id_cell.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let ident = syn::Ident::new(&format!("stream_{}", next_id), Span::call_site());
        let port = self.next_port();
        let source_pipeline = Self::gen_source_statement(self, &port);

        builders
            .borrow_mut()
            .as_mut()
            .unwrap()
            .entry(self.id())
            .or_default()
            .add_statement(parse_quote! {
                #ident = #source_pipeline -> tee();
            });

        (
            port,
            Stream {
                ident,
                node: self.clone(),
                next_id: next_id_cell,
                builders,
                is_delta: false,
                _phantom: PhantomData,
            },
        )
    }

    fn many_source_external<S: Location<'a>>(
        &self,
    ) -> (
        Self::Port,
        Stream<'a, Result<BytesMut, io::Error>, Async, Self>,
    )
    where
        S: HfSendOneToMany<'a, Self>,
    {
        let (next_id_cell, builders) = self.flow_builder();

        let next_id = {
            let mut next_id = next_id_cell.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let ident = syn::Ident::new(&format!("stream_{}", next_id), Span::call_site());
        let port = self.next_port();
        let source_pipeline = S::gen_source_statement(self, &port);

        builders
            .borrow_mut()
            .as_mut()
            .unwrap()
            .entry(self.id())
            .or_default()
            .add_statement(parse_quote! {
                #ident = #source_pipeline -> tee();
            });

        (
            port,
            Stream {
                ident,
                node: self.clone(),
                next_id: next_id_cell,
                builders,
                is_delta: false,
                _phantom: PhantomData,
            },
        )
    }

    fn source_iter<T, E: IntoIterator<Item = T>>(
        &self,
        e: impl Quoted<'a, E>,
    ) -> Stream<'a, T, Windowed, Self> {
        let (next_id_cell, builders) = self.flow_builder();

        let next_id = {
            let mut next_id = next_id_cell.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let ident = syn::Ident::new(&format!("stream_{}", next_id), Span::call_site());
        let e = e.splice();

        builders
            .borrow_mut()
            .as_mut()
            .unwrap()
            .entry(self.id())
            .or_default()
            .add_statement(parse_quote! {
                #ident = source_iter(#e) -> tee();
            });

        Stream {
            ident,
            node: self.clone(),
            next_id: next_id_cell,
            builders,
            is_delta: false,
            _phantom: PhantomData,
        }
    }

    fn source_interval(
        &self,
        interval: impl Quoted<'a, Duration> + Copy + 'a,
    ) -> Stream<'a, hydroflow::tokio::time::Instant, Async, Self> {
        let (next_id_cell, builders) = self.flow_builder();

        let next_id = {
            let mut next_id = next_id_cell.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let ident = syn::Ident::new(&format!("stream_{}", next_id), Span::call_site());
        let interval = interval.splice();

        builders
            .borrow_mut()
            .as_mut()
            .unwrap()
            .entry(self.id())
            .or_default()
            .add_statement(parse_quote! {
                #ident = source_interval(#interval) -> tee();
            });

        Stream {
            ident,
            node: self.clone(),
            next_id: next_id_cell,
            builders,
            is_delta: false,
            _phantom: PhantomData,
        }
    }

    fn cycle<T, W>(&self) -> (HfCycle<'a, T, W, Self>, Stream<'a, T, W, Self>) {
        let (next_id_cell, builders) = self.flow_builder();

        let next_id = {
            let mut next_id = next_id_cell.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let ident = syn::Ident::new(&format!("stream_{}", next_id), Span::call_site());

        builders
            .borrow_mut()
            .as_mut()
            .unwrap()
            .entry(self.id())
            .or_default()
            .add_statement(parse_quote! {
                #ident = tee();
            });

        (
            HfCycle {
                ident: ident.clone(),
                node: self.clone(),
                builders,
                _phantom: PhantomData,
            },
            Stream {
                ident,
                node: self.clone(),
                next_id: next_id_cell,
                builders,
                is_delta: false,
                _phantom: PhantomData,
            },
        )
    }
}

pub trait Cluster<'a>: Location<'a> {
    fn ids<'b>(&'b self) -> impl Quoted<'a, &'a Vec<u32>> + Copy + 'a;
}
