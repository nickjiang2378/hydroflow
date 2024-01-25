use quote::quote_spanned;

use super::{
    DelayType, OperatorCategory, OperatorConstraints,
    OperatorWriteOutput, Persistence, WriteContextArgs, RANGE_0, RANGE_1,
};
use crate::diagnostic::{Diagnostic, Level};
use crate::graph::{OpInstGenerics, OperatorInstance, GraphEdgeType};

/// > 1 input stream, 1 output stream
///
/// > Arguments: a closure which itself takes two arguments:
/// an ‘accumulator’, and an element. The closure returns the value that the accumulator should have for the next iteration.
///
/// Akin to Rust's built-in reduce operator. Folds every element into an accumulator by applying a closure,
/// returning the final result.
///
/// > Note: The closure has access to the [`context` object](surface_flows.md#the-context-object).
///
/// `reduce` can also be provided with one generic lifetime persistence argument, either
/// `'tick` or `'static`, to specify how data persists. With `'tick`, elements will only be collected
/// within the same tick. With `'static`, the accumulator will be remembered across ticks and elements
/// are aggregated with elements arriving in later ticks. When not explicitly specified persistence
/// defaults to `'tick`.
///
/// ```hydroflow
/// source_iter([1,2,3,4,5])
///     -> reduce::<'tick>(|accum: &mut _, elem| {
///         *accum *= elem;
///     })
///     -> assert_eq([120]);
/// ```
pub const REDUCE: OperatorConstraints = OperatorConstraints {
    name: "reduce",
    categories: &[OperatorCategory::Fold],
    hard_range_inn: RANGE_1,
    soft_range_inn: RANGE_1,
    hard_range_out: RANGE_1,
    soft_range_out: RANGE_1,
    num_args: 1,
    persistence_args: &(0..=1),
    type_args: RANGE_0,
    is_external_input: false,
    ports_inn: None,
    ports_out: None,
    input_delaytype_fn: |_| Some(DelayType::Stratum),
    input_edgetype_fn: |_| Some(GraphEdgeType::Value),
    output_edgetype_fn: |_| GraphEdgeType::Value,
    flow_prop_fn: None,
    write_fn: |wc @ &WriteContextArgs {
                   context,
                   hydroflow,
                   op_span,
                   ident,
                   inputs,
                   is_pull,
                   op_inst:
                       OperatorInstance {
                           arguments,
                           generics:
                               OpInstGenerics {
                                   persistence_args, ..
                               },
                           ..
                       },
                   ..
               },
               diagnostics| {
        assert!(is_pull);

        let persistence = match persistence_args[..] {
            [] => Persistence::Tick,
            [a] => a,
            _ => unreachable!(),
        };

        let input = &inputs[0];
        let func = &arguments[0];
        let reducedata_ident = wc.make_ident("reducedata_ident");
        let accumulator_ident = wc.make_ident("accumulator");
        let ret_ident = wc.make_ident("ret");
        let iterator_item_ident = wc.make_ident("iterator_item");

        let (write_prologue, write_iterator, write_iterator_after) = match persistence {
            Persistence::Tick => (
                Default::default(),
                quote_spanned! {op_span=>
                    let #ident = {
                        let mut #input = #input;
                        let #accumulator_ident = #input.next();

                        #[inline(always)]
                        /// A: accumulator type
                        /// O: output type
                        fn call_comb_type<A, O>(acc: &mut A, item: A, f: impl Fn(&mut A, A) -> O) -> O {
                            f(acc, item)
                        }

                        if let ::std::option::Option::Some(mut #accumulator_ident) = #accumulator_ident {
                            for #iterator_item_ident in #input {
                                #[allow(clippy::redundant_closure_call)]
                                call_comb_type(&mut #accumulator_ident, #iterator_item_ident, #func);
                            }

                            ::std::option::Option::Some(#accumulator_ident)
                        } else {
                            ::std::option::Option::None
                        }.into_iter()
                    };
                },
                Default::default(),
            ),
            Persistence::Static => (
                quote_spanned! {op_span=>
                    let #reducedata_ident = #hydroflow.add_state(
                        ::std::cell::Cell::new(::std::option::Option::None)
                    );
                },
                quote_spanned! {op_span=>
                    let #ident = {
                        let mut #input = #input;
                        let #accumulator_ident = if let ::std::option::Option::Some(#accumulator_ident) = #context.state_ref(#reducedata_ident).take() {
                            Some(#accumulator_ident)
                        } else {
                            #input.next()
                        };

                        #[inline(always)]
                        /// A: accumulator type
                        /// O: output type
                        fn call_comb_type<A, O>(acc: &mut A, item: A, f: impl Fn(&mut A, A) -> O) -> O {
                            f(acc, item)
                        }

                        let #ret_ident = if let ::std::option::Option::Some(mut #accumulator_ident) = #accumulator_ident {
                            for #iterator_item_ident in #input {
                                #[allow(clippy::redundant_closure_call)]
                                call_comb_type(&mut #accumulator_ident, #iterator_item_ident, #func);
                            }

                            ::std::option::Option::Some(#accumulator_ident)
                        } else {
                            ::std::option::Option::None
                        };

                        #context.state_ref(#reducedata_ident).set(::std::clone::Clone::clone(&#ret_ident));

                        #ret_ident.into_iter()
                    };
                },
                quote_spanned! {op_span=>
                    #context.schedule_subgraph(#context.current_subgraph(), false);
                },
            ),
            Persistence::Mutable => {
                diagnostics.push(Diagnostic::spanned(
                    op_span,
                    Level::Error,
                    "An implementation of 'mutable does not exist",
                ));
                return Err(());
            }
        };

        Ok(OperatorWriteOutput {
            write_prologue,
            write_iterator,
            write_iterator_after,
        })
    },
};
