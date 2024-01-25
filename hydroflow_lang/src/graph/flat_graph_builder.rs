//! Build a flat graph from [`HfStatement`]s.

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use proc_macro2::Span;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Error, Ident, ItemUse};

use super::ops::find_op_op_constraints;
use super::{GraphNode, GraphNodeId, HydroflowGraph, PortIndexValue};
use crate::diagnostic::{Diagnostic, Level};
use crate::graph::ops::{PortListSpec, RangeTrait};
use crate::parse::{HfCode, HfStatement, Operator, Pipeline};
use crate::pretty_span::PrettySpan;

#[derive(Clone, Debug)]
struct Ends {
    inn: Option<(PortIndexValue, GraphDet)>,
    out: Option<(PortIndexValue, GraphDet)>,
}

#[derive(Clone, Debug)]
enum GraphDet {
    Determined(GraphNodeId),
    Undetermined(Ident),
}

/// Wraper around [`HydroflowGraph`] to build a flat graph from AST code.
#[derive(Debug, Default)]
pub struct FlatGraphBuilder {
    /// Spanned error/warning/etc diagnostics to emit.
    diagnostics: Vec<Diagnostic>,

    /// HydroflowGraph being built.
    flat_graph: HydroflowGraph,
    /// Variable names, used as [`HfStatement::Named`] are added.
    /// Value will be set to `Err(())` if the name references an illegal self-referential cycle.
    varname_ends: BTreeMap<Ident, Result<Ends, ()>>,
    /// Each (out -> inn) link inputted.
    links: Vec<Ends>,

    /// Use statements.
    uses: Vec<ItemUse>,

    /// In order to make import!() statements relative to the current file, we need to know where the file is that is building the flat graph.
    invocating_file_path: PathBuf,

    /// If the flat graph is being loaded as a module, then two initial ModuleBoundary nodes are inserted into the graph. One
    /// for the input into the module and one for the output out of the module.
    module_boundary_nodes: Option<(GraphNodeId, GraphNodeId)>,
}

impl FlatGraphBuilder {
    /// Create a new empty graph builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Convert the Hydroflow code AST into a graph builder.
    pub fn from_hfcode(input: HfCode, macro_invocation_path: PathBuf) -> Self {
        let mut builder = Self {
            invocating_file_path: macro_invocation_path,
            ..Default::default()
        };
        builder.process_statements(input.statements);

        builder
    }

    /// Convert the Hydroflow code AST into a graph builder.
    pub fn from_hfmodule(input: HfCode, root_path: PathBuf) -> Self {
        let mut builder = Self::default();
        builder.invocating_file_path = root_path; // imports inside of modules should be relative to the importing file.
        builder.module_boundary_nodes = Some((
            builder.flat_graph.insert_node(
                GraphNode::ModuleBoundary {
                    input: true,
                    import_expr: Span::call_site(),
                },
                Some(Ident::new("input", Span::call_site())),
            ),
            builder.flat_graph.insert_node(
                GraphNode::ModuleBoundary {
                    input: false,
                    import_expr: Span::call_site(),
                },
                Some(Ident::new("output", Span::call_site())),
            ),
        ));
        builder.process_statements(input.statements);
        builder
    }

    fn process_statements(&mut self, statements: impl IntoIterator<Item = HfStatement>) {
        for stmt in statements {
            self.add_statement(stmt);
        }
    }

    /// Build into an unpartitioned [`HydroflowGraph`], returning a tuple of a `HydroflowGraph` and
    /// any diagnostics.
    ///
    /// Even if there are errors, the `HydroflowGraph` will be returned (potentially in a invalid
    /// state). Does not call `emit` on any diagnostics.
    pub fn build(mut self) -> (HydroflowGraph, Vec<ItemUse>, Vec<Diagnostic>) {
        self.connect_operator_links();
        self.process_operator_errors();

        (self.flat_graph, self.uses, self.diagnostics)
    }

    /// Add a single [`HfStatement`] line to this `HydroflowGraph`.
    pub fn add_statement(&mut self, stmt: HfStatement) {
        match stmt {
            HfStatement::Use(yuse) => {
                self.uses.push(yuse);
            }
            HfStatement::Named(named) => {
                let stmt_span = named.span();
                let ends = self.add_pipeline(named.pipeline, Some(&named.name));
                match self.varname_ends.entry(named.name) {
                    Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(Ok(ends));
                    }
                    Entry::Occupied(occupied_entry) => {
                        let prev_conflict = occupied_entry.key();
                        self.diagnostics.push(Diagnostic::spanned(
                            stmt_span,
                            Level::Error,
                            format!(
                                "Name assignment to `{}` conflicts with existing assignment: {} (1/2)",
                                prev_conflict,
                                PrettySpan(prev_conflict.span())
                            ),
                        ));
                        self.diagnostics.push(Diagnostic::spanned(
                            prev_conflict.span(),
                            Level::Error,
                            format!(
                                "Existing assignment to `{}` conflicts with later assignment: {} (2/2)",
                                prev_conflict,
                                PrettySpan(stmt_span),
                            ),
                        ));
                    }
                }
            }
            HfStatement::Pipeline(pipeline_stmt) => {
                self.add_pipeline(pipeline_stmt.pipeline, None);
            }
        }
    }

    /// Helper: Add a pipeline, i.e. `a -> b -> c`. Return the input and output ends for it.
    fn add_pipeline(&mut self, pipeline: Pipeline, current_varname: Option<&Ident>) -> Ends {
        match pipeline {
            Pipeline::Paren(ported_pipeline_paren) => {
                let (inn_port, pipeline_paren, out_port) =
                    PortIndexValue::from_ported(ported_pipeline_paren);
                let og_ends = self.add_pipeline(*pipeline_paren.pipeline, current_varname);
                Self::helper_combine_ends(&mut self.diagnostics, og_ends, inn_port, out_port)
            }
            Pipeline::Name(pipeline_name) => {
                let (inn_port, ident, out_port) = PortIndexValue::from_ported(pipeline_name);

                // We could lookup non-forward references immediately, but easier to just have one
                // consistent code path. -mingwei
                Ends {
                    inn: Some((inn_port, GraphDet::Undetermined(ident.clone()))),
                    out: Some((out_port, GraphDet::Undetermined(ident))),
                }
            }
            Pipeline::ModuleBoundary(pipeline_name) => {
                let Some((input_node, output_node)) = self.module_boundary_nodes else {
                    self.diagnostics.push(
                        Error::new(
                            pipeline_name.span(),
                            "`mod` is only usable inside of a module.",
                        )
                        .into(),
                    );

                    return Ends {
                        inn: None,
                        out: None,
                    };
                };

                let (inn_port, _, out_port) = PortIndexValue::from_ported(pipeline_name);

                Ends {
                    inn: Some((inn_port, GraphDet::Determined(output_node))),
                    out: Some((out_port, GraphDet::Determined(input_node))),
                }
            }
            Pipeline::Link(pipeline_link) => {
                // Add the nested LHS and RHS of this link.
                let lhs_ends = self.add_pipeline(*pipeline_link.lhs, current_varname);
                let rhs_ends = self.add_pipeline(*pipeline_link.rhs, current_varname);

                // Outer (first and last) ends.
                let outer_ends = Ends {
                    inn: lhs_ends.inn,
                    out: rhs_ends.out,
                };
                // Inner (link) ends.
                let link_ends = Ends {
                    out: lhs_ends.out,
                    inn: rhs_ends.inn,
                };
                self.links.push(link_ends);
                outer_ends
            }
            Pipeline::Operator(operator) => {
                let op_span = Some(operator.span());
                let nid = self
                    .flat_graph
                    .insert_node(GraphNode::Operator(operator), current_varname.cloned());
                Ends {
                    inn: Some((PortIndexValue::Elided(op_span), GraphDet::Determined(nid))),
                    out: Some((PortIndexValue::Elided(op_span), GraphDet::Determined(nid))),
                }
            }
            Pipeline::Import(import) => {
                // TODO: https://github.com/rust-lang/rfcs/pull/3200
                // this would be way better...
                let file_path = {
                    let mut dir = self.invocating_file_path.clone();
                    dir.pop();
                    dir.join(import.filename.value())
                };

                let file_contents = match std::fs::read_to_string(&file_path) {
                    Ok(contents) => contents,
                    Err(err) => {
                        self.diagnostics.push(Diagnostic::spanned(
                            import.filename.span(),
                            Level::Error,
                            format!("filename: {}, err: {err}", import.filename.value()),
                        ));

                        return Ends {
                            inn: None,
                            out: None,
                        };
                    }
                };

                let statements = match syn::parse_str::<HfCode>(&file_contents) {
                    Ok(code) => code,
                    Err(err) => {
                        self.diagnostics.push(Diagnostic::spanned(
                            import.span(),
                            Level::Error,
                            err.to_string(),
                        ));

                        return Ends {
                            inn: None,
                            out: None,
                        };
                    }
                };

                let flat_graph_builder =
                    crate::graph::FlatGraphBuilder::from_hfmodule(statements, file_path);
                let (flat_graph, _uses, diagnostics) = flat_graph_builder.build();
                diagnostics
                    .iter()
                    .for_each(crate::diagnostic::Diagnostic::emit);

                self.merge_in(flat_graph, import.span())
            }
        }
    }

    /// Merge one flatgraph into the current flatgraph
    /// other must be a flatgraph and not be partitioned yet.
    fn merge_in(&mut self, other: HydroflowGraph, parent_span: Span) -> Ends {
        assert_eq!(other.subgraphs().count(), 0);

        let mut ends = Ends {
            inn: None,
            out: None,
        };

        let mut node_mapping = BTreeMap::new();

        for (other_node_id, node) in other.nodes() {
            match node {
                GraphNode::Operator(_) => {
                    let varname = other.node_varname(other_node_id);
                    let new_id = self.flat_graph.insert_node(node.clone(), varname);
                    node_mapping.insert(other_node_id, new_id);
                }
                GraphNode::ModuleBoundary { input, .. } => {
                    let new_id = self.flat_graph.insert_node(
                        GraphNode::ModuleBoundary {
                            input: *input,
                            import_expr: parent_span,
                        },
                        Some(Ident::new(&format!("module_{}", input), parent_span)),
                    );
                    node_mapping.insert(other_node_id, new_id);

                    // in the case of nested imports, this module boundary might not be the module boundary into or out of the top-most module
                    // So we have to be careful to only target those two boundaries.
                    // There should be no inputs to it, if it is an input boundary, if it is the top-most one.
                    // and there should be no outputs from it, if it is an output boundary, if it is the top-most one.
                    if *input && other.node_predecessor_nodes(other_node_id).count() == 0 {
                        if other.node_predecessor_nodes(other_node_id).count() == 0 {
                            ends.inn =
                                Some((PortIndexValue::Elided(None), GraphDet::Determined(new_id)));
                        }
                    } else if !(*input) && other.node_successor_nodes(other_node_id).count() == 0 {
                        ends.out =
                            Some((PortIndexValue::Elided(None), GraphDet::Determined(new_id)));
                    }
                }
                GraphNode::Handoff { .. } => {
                    panic!("Handoff in graph that is being merged into self")
                }
            }
        }

        for (other_edge_id, (other_src, other_dst)) in other.edges() {
            let (src_port, dst_port) = other.edge_ports(other_edge_id);
            let edge_type = other.edge_type(other_edge_id);

            let new_edge_id = self.flat_graph.insert_edge(
                *node_mapping.get(&other_src).unwrap(),
                src_port.clone(),
                *node_mapping.get(&other_dst).unwrap(),
                dst_port.clone(),
            );
            if let Some(edge_type) = edge_type {
                self.flat_graph.insert_edge_type(new_edge_id, edge_type);
            }
        }

        ends
    }

    /// Connects operator links as a final building step. Processes all the links stored in
    /// `self.links` and actually puts them into the graph.
    fn connect_operator_links(&mut self) {
        for Ends { out, inn } in std::mem::take(&mut self.links) {
            let out_opt = self.helper_resolve_name(out, false);
            let inn_opt = self.helper_resolve_name(inn, true);
            // `None` already have errors in `self.diagnostics`.
            if let (Some((out_port, out_node)), Some((inn_port, inn_node))) = (out_opt, inn_opt) {
                self.connect_operators(out_port, out_node, inn_port, inn_node);
            }
        }
    }
    /// Recursively resolve a variable name. For handling forward (and backward) name references
    /// after all names have been assigned.
    /// Returns `None` if the name is not resolvable, either because it was never assigned or
    /// because it contains a self-referential cycle.
    fn helper_resolve_name(
        &mut self,
        mut port_det: Option<(PortIndexValue, GraphDet)>,
        is_in: bool,
    ) -> Option<(PortIndexValue, GraphNodeId)> {
        const BACKUP_RECURSION_LIMIT: usize = 1024;

        let mut names = Vec::new();
        for _ in 0..BACKUP_RECURSION_LIMIT {
            match port_det? {
                (port, GraphDet::Determined(node_id)) => {
                    return Some((port, node_id));
                }
                (port, GraphDet::Undetermined(ident)) => {
                    let Some(name_ends_result) = self.varname_ends.get(&ident) else {
                        self.diagnostics.push(Diagnostic::spanned(
                            ident.span(),
                            Level::Error,
                            format!("Cannot find name `{}`; name was never assigned.", ident),
                        ));
                        return None;
                    };
                    // Check for a self-referential cycle.
                    let cycle_found = names.contains(&ident);
                    if !cycle_found {
                        names.push(ident);
                    };
                    if cycle_found || name_ends_result.is_err() {
                        let len = names.len();
                        for (i, name) in names.into_iter().enumerate() {
                            self.diagnostics.push(Diagnostic::spanned(
                                name.span(),
                                Level::Error,
                                format!(
                                    "Name `{}` forms or references an illegal self-referential cycle ({}/{}).",
                                    name,
                                    i + 1,
                                    len
                                ),
                            ));
                            // Set value as `Err(())` to trigger `name_ends_result.is_err()`
                            // diagnostics above if the name is referenced in the future.
                            self.varname_ends.insert(name, Err(()));
                        }
                        return None;
                    }

                    // No self-cycle.
                    let name_ends = name_ends_result.as_ref().unwrap();
                    let prev = if is_in {
                        &name_ends.inn
                    } else {
                        &name_ends.out
                    };
                    port_det = Self::helper_combine_end(
                        &mut self.diagnostics,
                        prev.clone(),
                        port,
                        if is_in { "input" } else { "output" },
                    );
                }
            }
        }
        self.diagnostics.push(Diagnostic::spanned(
            Span::call_site(),
            Level::Error,
            format!(
                "Reached the recursion limit {} while resolving names. This is either a hydroflow bug or you have an absurdly long chain of names: `{}`.",
                BACKUP_RECURSION_LIMIT,
                names.iter().map(ToString::to_string).collect::<Vec<_>>().join("` -> `"),
            )
        ));
        None
    }
    /// Connect two operators on the given port indexes.
    fn connect_operators(
        &mut self,
        src_port: PortIndexValue,
        src: GraphNodeId,
        dst_port: PortIndexValue,
        dst: GraphNodeId,
    ) {
        {
            /// Helper to emit conflicts when a port is used twice.
            fn emit_conflict(
                inout: &str,
                old: &PortIndexValue,
                new: &PortIndexValue,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                // TODO(mingwei): Use `MultiSpan` once `proc_macro2` supports it.
                diagnostics.push(Diagnostic::spanned(
                    old.span(),
                    Level::Error,
                    format!(
                        "{} connection conflicts with below ({}) (1/2)",
                        inout,
                        PrettySpan(new.span()),
                    ),
                ));
                diagnostics.push(Diagnostic::spanned(
                    new.span(),
                    Level::Error,
                    format!(
                        "{} connection conflicts with above ({}) (2/2)",
                        inout,
                        PrettySpan(old.span()),
                    ),
                ));
            }

            // Handle src's successor port conflicts:
            if src_port.is_specified() {
                for conflicting_port in self
                    .flat_graph
                    .node_successor_edges(src)
                    .map(|edge_id| self.flat_graph.edge_ports(edge_id).0)
                    .filter(|&port| port == &src_port)
                {
                    emit_conflict("Output", conflicting_port, &src_port, &mut self.diagnostics);
                }
            }

            // Handle dst's predecessor port conflicts:
            if dst_port.is_specified() {
                for conflicting_port in self
                    .flat_graph
                    .node_predecessor_edges(dst)
                    .map(|edge_id| self.flat_graph.edge_ports(edge_id).1)
                    .filter(|&port| port == &dst_port)
                {
                    emit_conflict("Input", conflicting_port, &dst_port, &mut self.diagnostics);
                }
            }
        }
        self.flat_graph.insert_edge(src, src_port, dst, dst_port);
    }

    /// Process operators and emit operator errors.
    fn process_operator_errors(&mut self) {
        self.make_operator_instances();
        self.insert_operator_edge_types();
        self.check_operator_errors();
    }

    /// Make `OperatorInstance`s for each operator node.
    fn make_operator_instances(&mut self) {
        self.flat_graph
            .insert_node_op_insts_all(&mut self.diagnostics);
    }

    /// Find and insert operator [`GraphEdgeType`]s for edges.
    fn insert_operator_edge_types(&mut self) {
        for edge_id in self.flat_graph.edge_ids().collect::<Vec<_>>() {
            let (src, _dst) = self.flat_graph.edge(edge_id);
            match self.flat_graph.node(src) {
                GraphNode::Operator(_) => {
                    let Some(src_op_inst) = self.flat_graph.node_op_inst(src) else {
                        continue;
                    };
                    let (src_port, _dst_port) = self.flat_graph.edge_ports(edge_id);
                    let edge_type = (src_op_inst.op_constraints.output_edgetype_fn)(src_port);
                    let _old_edge_type = self.flat_graph.insert_edge_type(edge_id, edge_type);
                    // _old_edge_type should usually be `None`? Except from modules?
                }
                GraphNode::Handoff { .. } => {
                    // TODO(mingwei)
                    // // This is still a flat graph - there should generally not be handoffs.
                    // // Handoffs can only handle value edges.
                    // self.flat_graph
                    //     .insert_edge_type(edge_id, GraphEdgeType::Value);
                    unimplemented!();
                }
                GraphNode::ModuleBoundary { .. } => {
                    // No-op. Handle when the module is connected.
                }
            }
        }
    }

    /// Validates that operators have valid number of inputs, outputs, & arguments.
    /// Adds errors (and warnings) to `self.diagnostics`.
    fn check_operator_errors(&mut self) {
        for (node_id, node) in self.flat_graph.nodes() {
            match node {
                GraphNode::Operator(operator) => {
                    let Some(op_constraints) = find_op_op_constraints(operator) else {
                        // Error already emitted by `insert_node_op_insts_all`.
                        continue;
                    };
                    // Check number of args
                    if op_constraints.num_args != operator.args.len() {
                        self.diagnostics.push(Diagnostic::spanned(
                            operator.span(),
                            Level::Error,
                            format!(
                                "expected {} argument(s), found {}",
                                op_constraints.num_args,
                                operator.args.len()
                            ),
                        ));
                    }

                    // Check input/output (port) arity
                    /// Returns true if an error was found.
                    fn emit_arity_error(
                        operator: &Operator,
                        is_in: bool,
                        is_hard: bool,
                        degree: usize,
                        range: &dyn RangeTrait<usize>,
                        diagnostics: &mut Vec<Diagnostic>,
                    ) -> bool {
                        let op_name = &*operator.name_string();
                        let message = format!(
                            "`{}` {} have {} {}, actually has {}.",
                            op_name,
                            if is_hard { "must" } else { "should" },
                            range.human_string(),
                            if is_in { "input(s)" } else { "output(s)" },
                            degree,
                        );
                        let out_of_range = !range.contains(&degree);
                        if out_of_range {
                            diagnostics.push(Diagnostic::spanned(
                                operator.span(),
                                if is_hard {
                                    Level::Error
                                } else {
                                    Level::Warning
                                },
                                message,
                            ));
                        }
                        out_of_range
                    }

                    let inn_degree = self.flat_graph.node_degree_in(node_id);
                    let _ = emit_arity_error(
                        operator,
                        true,
                        true,
                        inn_degree,
                        op_constraints.hard_range_inn,
                        &mut self.diagnostics,
                    ) || emit_arity_error(
                        operator,
                        true,
                        false,
                        inn_degree,
                        op_constraints.soft_range_inn,
                        &mut self.diagnostics,
                    );

                    let out_degree = self.flat_graph.node_degree_out(node_id);
                    let _ = emit_arity_error(
                        operator,
                        false,
                        true,
                        out_degree,
                        op_constraints.hard_range_out,
                        &mut self.diagnostics,
                    ) || emit_arity_error(
                        operator,
                        false,
                        false,
                        out_degree,
                        op_constraints.soft_range_out,
                        &mut self.diagnostics,
                    );

                    fn emit_port_error<'a>(
                        operator_span: Span,
                        expected_ports_fn: Option<fn() -> PortListSpec>,
                        actual_ports_iter: impl Iterator<Item = &'a PortIndexValue>,
                        input_output: &'static str,
                        diagnostics: &mut Vec<Diagnostic>,
                    ) {
                        let Some(expected_ports_fn) = expected_ports_fn else {
                            return;
                        };
                        let PortListSpec::Fixed(expected_ports) = (expected_ports_fn)() else {
                            // Separate check inside of `demux` special case.
                            return;
                        };
                        let expected_ports: Vec<_> = expected_ports.into_iter().collect();

                        // Reject unexpected ports.
                        let ports: BTreeSet<_> = actual_ports_iter
                            // Use `inspect` before collecting into `BTreeSet` to ensure we get
                            // both error messages on duplicated port names.
                            .inspect(|actual_port_iv| {
                                // For each actually used port `port_index_value`, check if it is expected.
                                let is_expected = expected_ports.iter().any(|port_index| {
                                    actual_port_iv == &&port_index.clone().into()
                                });
                                // If it is not expected, emit a diagnostic error.
                                if !is_expected {
                                    diagnostics.push(Diagnostic::spanned(
                                        actual_port_iv.span(),
                                        Level::Error,
                                        format!(
                                            "Unexpected {} port: {}. Expected one of: `{}`",
                                            input_output,
                                            actual_port_iv.as_error_message_string(),
                                            itertools::Itertools::intersperse(
                                                expected_ports
                                                    .iter()
                                                    .map(|port| Cow::Owned(
                                                        port.to_token_stream().to_string()
                                                    )),
                                                Cow::Borrowed("`, `")
                                            ).collect::<String>()
                                        ),
                                    ))
                                }
                            })
                            .collect();

                        // List missing expected ports.
                        let missing: Vec<_> = expected_ports
                            .into_iter()
                            .filter_map(|expected_port| {
                                let tokens = expected_port.to_token_stream();
                                if !ports.contains(&&expected_port.into()) {
                                    Some(tokens)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !missing.is_empty() {
                            diagnostics.push(Diagnostic::spanned(
                                operator_span,
                                Level::Error,
                                format!(
                                    "Missing expected {} port(s): `{}`.",
                                    input_output,
                                    itertools::Itertools::intersperse(
                                        missing.into_iter().map(|port| Cow::Owned(
                                            port.to_token_stream().to_string()
                                        )),
                                        Cow::Borrowed("`, `")
                                    )
                                    .collect::<String>()
                                ),
                            ));
                        }
                    }

                    emit_port_error(
                        operator.span(),
                        op_constraints.ports_inn,
                        self.flat_graph
                            .node_predecessor_edges(node_id)
                            .map(|edge_id| self.flat_graph.edge_ports(edge_id).1),
                        "input",
                        &mut self.diagnostics,
                    );
                    emit_port_error(
                        operator.span(),
                        op_constraints.ports_out,
                        self.flat_graph
                            .node_successor_edges(node_id)
                            .map(|edge_id| self.flat_graph.edge_ports(edge_id).0),
                        "output",
                        &mut self.diagnostics,
                    );

                    // Check edge types.
                    {
                        for (edge_id, prev_node_id) in self.flat_graph.node_predecessors(node_id) {
                            {
                                // Module boundaries will not have an edge type.
                                if matches!(
                                    self.flat_graph.node(prev_node_id),
                                    GraphNode::ModuleBoundary { .. }
                                ) {
                                    continue;
                                }
                                // Skip if previous node is unknown. // TODO(mingwei): handle explicit handoffs if we add them.
                                if self.flat_graph.node_op_inst(prev_node_id).is_none() {
                                    continue;
                                }
                            }

                            let port_in = self.flat_graph.edge_ports(edge_id).0;
                            let Some(edge_type_expected) =
                                (op_constraints.input_edgetype_fn)(port_in)
                            else {
                                // `None` means any edge type is allowed.
                                continue;
                            };
                            let Some(edge_type_actual) = self.flat_graph.edge_type(edge_id) else {
                                self.diagnostics.push(Diagnostic::spanned(
                                    port_in.span(),
                                    Level::Error,
                                    "Operator input has no edge type, this is a Hydroflow bug.",
                                ));
                                continue;
                            };
                            if edge_type_expected != edge_type_actual {
                                self.diagnostics.push(Diagnostic::spanned(
                                    port_in.span(),
                                    Level::Error,
                                    format!(
                                        "Operator requires a {:?} edge type input, but received a {:?} edge type input.",
                                        edge_type_expected,
                                        edge_type_actual,
                                    ),
                                ));
                            }
                        }
                    }
                }
                GraphNode::Handoff { .. } => todo!("Node::Handoff"),
                GraphNode::ModuleBoundary { .. } => {
                    // Module boundaries don't require any checking.
                }
            }
        }
    }

    /// Helper function.
    /// Combine the port indexing information for indexing wrapped around a name.
    /// Because the name may already have indexing, this may introduce double indexing (i.e. `[0][0]my_var[0][0]`)
    /// which would be an error.
    fn helper_combine_ends(
        diagnostics: &mut Vec<Diagnostic>,
        og_ends: Ends,
        inn_port: PortIndexValue,
        out_port: PortIndexValue,
    ) -> Ends {
        Ends {
            inn: Self::helper_combine_end(diagnostics, og_ends.inn, inn_port, "input"),
            out: Self::helper_combine_end(diagnostics, og_ends.out, out_port, "output"),
        }
    }

    /// Helper function.
    /// Combine the port indexing info for one input or output.
    fn helper_combine_end(
        diagnostics: &mut Vec<Diagnostic>,
        og: Option<(PortIndexValue, GraphDet)>,
        other: PortIndexValue,
        input_output: &'static str,
    ) -> Option<(PortIndexValue, GraphDet)> {
        // TODO(mingwei): minification pass over this code?

        let other_span = other.span();

        let (og_port, og_node) = og?;
        match og_port.combine(other) {
            Ok(combined_port) => Some((combined_port, og_node)),
            Err(og_port) => {
                // TODO(mingwei): Use `MultiSpan` once `proc_macro2` supports it.
                diagnostics.push(Diagnostic::spanned(
                    og_port.span(),
                    Level::Error,
                    format!(
                        "Indexing on {} is overwritten below ({}) (1/2).",
                        input_output,
                        PrettySpan(other_span),
                    ),
                ));
                diagnostics.push(Diagnostic::spanned(
                    other_span,
                    Level::Error,
                    format!(
                        "Cannot index on already-indexed {}, previously indexed above ({}) (2/2).",
                        input_output,
                        PrettySpan(og_port.span()),
                    ),
                ));
                // When errored, just use original and ignore OTHER port to minimize
                // noisy/extra diagnostics.
                Some((og_port, og_node))
            }
        }
    }
}
