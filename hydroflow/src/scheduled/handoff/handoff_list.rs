use ref_cast::RefCast;
use sealed::sealed;

use crate::scheduled::graph::HandoffData;
use crate::scheduled::port::{BaseCtx, BasePort};
use crate::scheduled::type_list::TypeList;
use crate::scheduled::{HandoffId, SubgraphId};

use super::Handoff;

#[sealed]
pub trait SendPortList: BasePortList<true> {}
#[sealed]
impl<T: BasePortList<true>> SendPortList for T {}

#[sealed]
pub trait RecvPortList: BasePortList<false> {}
#[sealed]
impl<T: BasePortList<false>> RecvPortList for T {}

#[sealed]
pub trait BasePortList<const S: bool>: TypeList {
    #[allow(clippy::ptr_arg)]
    fn set_graph_meta<'a>(
        &self,
        handoffs: &'a mut [HandoffData],
        pred: Option<SubgraphId>,
        succ: Option<SubgraphId>,
        out_handoff_ids: &mut Vec<HandoffId>,
    );

    type Ctx<'a>: TypeList;
    fn make_ctx<'a>(&self, handoffs: &'a [HandoffData]) -> Self::Ctx<'a>;
}
#[sealed]
impl<Rest, H, const S: bool> BasePortList<S> for (BasePort<H, S>, Rest)
where
    H: Handoff,
    Rest: BasePortList<S>,
{
    fn set_graph_meta<'a>(
        &self,
        handoffs: &'a mut [HandoffData],
        pred: Option<SubgraphId>,
        succ: Option<SubgraphId>,
        out_handoff_ids: &mut Vec<HandoffId>,
    ) {
        let (this, rest) = self;

        out_handoff_ids.push(this.handoff_id);

        let handoff = handoffs.get_mut(this.handoff_id).unwrap();
        if let Some(pred) = pred {
            handoff.preds.push(pred);
        }
        if let Some(succ) = succ {
            handoff.succs.push(succ);
        }
        rest.set_graph_meta(handoffs, pred, succ, out_handoff_ids);
    }

    type Ctx<'a> = (&'a BaseCtx<H, S>, Rest::Ctx<'a>);
    fn make_ctx<'a>(&self, handoffs: &'a [HandoffData]) -> Self::Ctx<'a> {
        let (this, rest) = self;
        let handoff = handoffs
            .get(this.handoff_id)
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
impl<const S: bool> BasePortList<S> for () {
    fn set_graph_meta<'a>(
        &self,
        _handoffs: &'a mut [HandoffData],
        _pred: Option<SubgraphId>,
        _succ: Option<SubgraphId>,
        _out_handoff_ids: &mut Vec<HandoffId>,
    ) {
    }

    type Ctx<'a> = ();
    fn make_ctx<'a>(&self, _handoffs: &'a [HandoffData]) -> Self::Ctx<'a> {}
}

#[sealed]
pub trait BasePortListSplit<A, const S: bool>: BasePortList<S>
where
    A: BasePortList<S>,
{
    type Suffix: BasePortList<S>;

    #[allow(clippy::needless_lifetimes)]
    fn split_ctx<'a>(
        ctx: Self::Ctx<'a>,
    ) -> (A::Ctx<'a>, <Self::Suffix as BasePortList<S>>::Ctx<'a>);
}
#[sealed]
impl<H, T, U, const S: bool> BasePortListSplit<(BasePort<H, S>, U), S> for (BasePort<H, S>, T)
where
    H: Handoff,
    T: BasePortListSplit<U, S>,
    U: BasePortList<S>,
{
    type Suffix = T::Suffix;

    #[allow(clippy::needless_lifetimes)]
    fn split_ctx<'a>(
        ctx: Self::Ctx<'a>,
    ) -> (
        <(BasePort<H, S>, U) as BasePortList<S>>::Ctx<'a>,
        <Self::Suffix as BasePortList<S>>::Ctx<'a>,
    ) {
        let (x, t) = ctx;
        let (u, v) = <T as BasePortListSplit<U, S>>::split_ctx(t);
        ((x, u), v)
    }
}
#[sealed]
impl<T, const S: bool> BasePortListSplit<(), S> for T
where
    T: BasePortList<S>,
{
    type Suffix = T;

    #[allow(clippy::needless_lifetimes)]
    fn split_ctx<'a>(ctx: Self::Ctx<'a>) -> ((), T::Ctx<'a>) {
        ((), ctx)
    }
}

/// A variadic list of Handoff types, represented using a lisp-style tuple structure.
///
/// This trait is sealed and not meant to be implemented or used directly. Instead tuple lists (which already implement this trait) should be used, for example:
/// ```ignore
/// type MyHandoffList = (VecHandoff<usize>, (VecHandoff<String>, (TeeingHandoff<u32>, ())));
/// ```
/// The [`tl!`] (tuple list) macro simplifies usage of this kind:
/// ```ignore
/// type MyHandoffList = tl!(VecHandoff<usize>, VecHandoff<String>, TeeingHandoff<u32>);
/// ```
#[sealed]
pub trait HandoffList: TypeList {}
#[sealed]
impl<H, L> HandoffList for (H, L)
where
    H: 'static + Handoff,
    L: HandoffList,
{
}
#[sealed]
impl HandoffList for () {}
