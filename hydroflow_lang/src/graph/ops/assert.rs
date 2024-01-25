use syn::parse_quote_spanned;

use super::{OperatorCategory, OperatorConstraints, WriteContextArgs, RANGE_0, RANGE_1};
use crate::graph::{GraphEdgeType, OperatorInstance};

/// > 1 input stream, 1 optional output stream
/// > Arguments: a predicate function that will be applied to each item in the stream
///
/// If the predicate returns false for any input item then the operator will panic at runtime.
///
/// ```hydroflow
/// source_iter([1, 2, 3])
///     -> assert(|x| *x > 0)
///     -> assert_eq([1, 2, 3]);
/// ```
pub const ASSERT: OperatorConstraints = OperatorConstraints {
    name: "assert",
    categories: &[OperatorCategory::Control],
    hard_range_inn: RANGE_1,
    soft_range_inn: RANGE_1,
    hard_range_out: &(0..=1),
    soft_range_out: &(0..=1),
    num_args: 1,
    persistence_args: RANGE_0,
    type_args: RANGE_0,
    is_external_input: false,
    ports_inn: None,
    ports_out: None,
    input_delaytype_fn: |_| None,
    input_edgetype_fn: |_| Some(GraphEdgeType::Value),
    output_edgetype_fn: |_| GraphEdgeType::Value,
    flow_prop_fn: None,
    write_fn: |wc @ &WriteContextArgs {
                   op_span, op_inst, ..
               },
               diagnostics| {
        let args = &op_inst.arguments[0];

        let arguments = parse_quote_spanned! {op_span=>
            |x| {
                // This is to help constrain the types so that type inference works nicely.
                fn __constrain_types<T>(f: impl Fn(&T) -> bool, x: &T) -> bool {
                    (f)(x)
                }
                assert!(__constrain_types(#args, x));
            }
        };

        let wc = WriteContextArgs {
            op_inst: &OperatorInstance {
                arguments,
                ..op_inst.clone()
            },
            ..wc.clone()
        };

        (super::inspect::INSPECT.write_fn)(&wc, diagnostics)
    },
};
