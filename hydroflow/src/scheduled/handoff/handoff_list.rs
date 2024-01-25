//! Module for variadic handoff port lists, [`PortList`].

use ref_cast::RefCast;
use sealed::sealed;
use variadics::{variadic_trait, Variadic};

use super::Handoff;
use crate::scheduled::graph::HandoffData;
use crate::scheduled::port::{Polarity, Port, PortCtx};
use crate::scheduled::{HandoffId, SubgraphId};

/// Sealed trait for variadic lists of ports.
///
/// See the [`variadics`] crate for the strategy we use to implement variadics in Rust.
#[sealed]
pub trait PortList<S>: Variadic
where
    S: Polarity,
{
    /// Iteratively/recursively set the graph metadata for each port in this list.
    fn set_graph_meta(
        &self,
        handoffs: &mut [HandoffData],
        pred: Option<SubgraphId>,
        succ: Option<SubgraphId>,
        out_handoff_ids: &mut Vec<HandoffId>,
    );

    /// The [`Variadic`] return type of [`Self::make_ctx`].
    type Ctx<'a>: Variadic;
    /// Iteratively/recursively construct a `Ctx` variadic list.
    fn make_ctx<'a>(&self, handoffs: &'a [HandoffData]) -> Self::Ctx<'a>;
}
#[sealed]
impl<S, Rest, H> PortList<S> for (Port<S, H>, Rest)
where
    S: Polarity,
    H: Handoff,
    Rest: PortList<S>,
{
    fn set_graph_meta(
        &self,
        handoffs: &mut [HandoffData],
        pred: Option<SubgraphId>,
        succ: Option<SubgraphId>,
        out_handoff_ids: &mut Vec<HandoffId>,
    ) {
        let (this, rest) = self;

        out_handoff_ids.push(this.handoff_id);

        let handoff = handoffs.get_mut(this.handoff_id.0).unwrap();
        if let Some(pred) = pred {
            handoff.preds.push(pred);
        }
        if let Some(succ) = succ {
            handoff.succs.push(succ);
        }
        rest.set_graph_meta(handoffs, pred, succ, out_handoff_ids);
    }

    type Ctx<'a> = (&'a PortCtx<S, H>, Rest::Ctx<'a>);
    fn make_ctx<'a>(&self, handoffs: &'a [HandoffData]) -> Self::Ctx<'a> {
        let (this, rest) = self;
        let handoff = handoffs
            .get(this.handoff_id.0)
            .unwrap()
            .handoff
            .any_ref()
            .downcast_ref()
            .expect("Attempted to cast handoff to wrong type.");

        let ctx = RefCast::ref_cast(handoff);
        let ctx_rest = rest.make_ctx(handoffs);
        (ctx, ctx_rest)
    }
}
#[sealed]
impl<S> PortList<S> for ()
where
    S: Polarity,
{
    fn set_graph_meta(
        &self,
        _handoffs: &mut [HandoffData],
        _pred: Option<SubgraphId>,
        _succ: Option<SubgraphId>,
        _out_handoff_ids: &mut Vec<HandoffId>,
    ) {
    }

    type Ctx<'a> = ();
    fn make_ctx<'a>(&self, _handoffs: &'a [HandoffData]) -> Self::Ctx<'a> {}
}

/// Trait for splitting a list of ports into two.
#[sealed]
pub trait PortListSplit<S, A>: PortList<S>
where
    S: Polarity,
    A: PortList<S>,
{
    /// The suffix, second half of the split.
    type Suffix: PortList<S>;

    /// Split the port list, returning the prefix and [`Self::Suffix`] as the two halves.
    fn split_ctx(ctx: Self::Ctx<'_>) -> (A::Ctx<'_>, <Self::Suffix as PortList<S>>::Ctx<'_>);
}
#[sealed]
impl<S, H, T, U> PortListSplit<S, (Port<S, H>, U)> for (Port<S, H>, T)
where
    S: Polarity,
    H: Handoff,
    T: PortListSplit<S, U>,
    U: PortList<S>,
{
    type Suffix = T::Suffix;

    fn split_ctx(
        ctx: Self::Ctx<'_>,
    ) -> (
        <(Port<S, H>, U) as PortList<S>>::Ctx<'_>,
        <Self::Suffix as PortList<S>>::Ctx<'_>,
    ) {
        let (x, t) = ctx;
        let (u, v) = T::split_ctx(t);
        ((x, u), v)
    }
}
#[sealed]
impl<S, T> PortListSplit<S, ()> for T
where
    S: Polarity,
    T: PortList<S>,
{
    type Suffix = T;

    fn split_ctx(ctx: Self::Ctx<'_>) -> ((), T::Ctx<'_>) {
        ((), ctx)
    }
}

variadic_trait! {
    /// A variadic list of Handoff types, represented using a lisp-style tuple structure.
    ///
    /// This trait is sealed and not meant to be implemented or used directly. Instead tuple lists (which already implement this trait) should be used, for example:
    /// ```ignore
    /// type MyHandoffList = (VecHandoff<usize>, (VecHandoff<String>, (TeeingHandoff<u32>, ())));
    /// ```
    /// The [`var_expr!`](crate::var) macro simplifies usage of this kind:
    /// ```ignore
    /// type MyHandoffList = var_expr!(VecHandoff<usize>, VecHandoff<String>, TeeingHandoff<u32>);
    /// ```
    #[sealed]
    pub variadic<T> HandoffList where T: 'static + Handoff {}
}
