use super::{PullBuild, PullBuildBase};

use crate::scheduled::handoff::handoff_list::{BasePortList, BasePortListSplit, RecvPortList};
use crate::scheduled::type_list::Extend;

pub struct ChainPullBuild<PrevA, PrevB>
where
    PrevA: PullBuild,
    PrevB: PullBuild<ItemOut = PrevA::ItemOut>,

    PrevA::InputHandoffs: Extend<PrevB::InputHandoffs>,
    <PrevA::InputHandoffs as Extend<PrevB::InputHandoffs>>::Extended: RecvPortList
        + BasePortListSplit<PrevA::InputHandoffs, false, Suffix = PrevB::InputHandoffs>,
{
    prev_a: PrevA,
    prev_b: PrevB,
}

impl<PrevA, PrevB> ChainPullBuild<PrevA, PrevB>
where
    PrevA: PullBuild,
    PrevB: PullBuild<ItemOut = PrevA::ItemOut>,

    PrevA::InputHandoffs: Extend<PrevB::InputHandoffs>,
    <PrevA::InputHandoffs as Extend<PrevB::InputHandoffs>>::Extended: RecvPortList
        + BasePortListSplit<PrevA::InputHandoffs, false, Suffix = PrevB::InputHandoffs>,
{
    pub fn new(prev_a: PrevA, prev_b: PrevB) -> Self {
        Self { prev_a, prev_b }
    }
}

impl<PrevA, PrevB> PullBuildBase for ChainPullBuild<PrevA, PrevB>
where
    PrevA: PullBuild,
    PrevB: PullBuild<ItemOut = PrevA::ItemOut>,

    PrevA::InputHandoffs: Extend<PrevB::InputHandoffs>,
    <PrevA::InputHandoffs as Extend<PrevB::InputHandoffs>>::Extended: RecvPortList
        + BasePortListSplit<PrevA::InputHandoffs, false, Suffix = PrevB::InputHandoffs>,
{
    type ItemOut = PrevA::ItemOut;
    type Build<'slf, 'hof> = std::iter::Chain<PrevA::Build<'slf, 'hof>, PrevB::Build<'slf, 'hof>>;
}

impl<PrevA, PrevB> PullBuild for ChainPullBuild<PrevA, PrevB>
where
    PrevA: PullBuild,
    PrevB: PullBuild<ItemOut = PrevA::ItemOut>,

    PrevA::InputHandoffs: Extend<PrevB::InputHandoffs>,
    <PrevA::InputHandoffs as Extend<PrevB::InputHandoffs>>::Extended: RecvPortList
        + BasePortListSplit<PrevA::InputHandoffs, false, Suffix = PrevB::InputHandoffs>,
{
    type InputHandoffs = <PrevA::InputHandoffs as Extend<PrevB::InputHandoffs>>::Extended;

    fn build<'slf, 'hof>(
        &'slf mut self,
        input: <Self::InputHandoffs as BasePortList<false>>::Ctx<'hof>,
    ) -> Self::Build<'slf, 'hof> {
        let (input_a, input_b) =
            <Self::InputHandoffs as BasePortListSplit<_, false>>::split_ctx(input);
        let iter_a = self.prev_a.build(input_a);
        let iter_b = self.prev_b.build(input_b);
        iter_a.chain(iter_b)
    }
}
