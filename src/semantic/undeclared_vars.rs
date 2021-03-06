use crate::semantic::hir::*;

use std::collections::HashSet;

use super::name_registry::NameRegistry;

fn check_trivial_expr(
    declarations_found: &HashSet<String>,
    function_name: &str,
    expr: &TypedTrivialHIRExpr,
) {
    match &expr.0 {
        TrivialHIRExpr::Variable(v) => {
            if declarations_found.get(v).is_none() {
                panic!("Variable {v} not found, function: {function_name}");
            }
        }
        _ => {}
    }
}

fn check_expr(declarations_found: &HashSet<String>, function_name: &str, expr: &HIRExpr) {
    match expr {
        HIRExpr::Trivial(e, ..) => {
            check_trivial_expr(declarations_found, function_name, e);
        }
        HIRExpr::BinaryOperation(lhs, _, rhs, ..) => {
            check_trivial_expr(declarations_found, function_name, lhs);
            check_trivial_expr(declarations_found, function_name, rhs);
        }
        HIRExpr::FunctionCall(func_expr, args, ..) => {
            check_trivial_expr(declarations_found, function_name, func_expr);
            for fun_arg in args {
                check_trivial_expr(declarations_found, function_name, fun_arg);
            }
        }
        HIRExpr::UnaryExpression(_, unary_expr, ..) => {
            check_trivial_expr(declarations_found, function_name, unary_expr);
        }
        HIRExpr::MemberAccess(member_expr, ..) => {
            check_trivial_expr(declarations_found, function_name, member_expr);
        }
        HIRExpr::Array(item_exprs, ..) => {
            for array_item in item_exprs {
                check_trivial_expr(&declarations_found, function_name, array_item);
            }
        }
        HIRExpr::Cast(expr, typedef, ..) => {
            check_trivial_expr(&declarations_found, function_name, expr)
        }
    }
}

fn detect_decl_errors_in_body(
    declarations_found: &mut HashSet<String>,
    function_name: &str,
    body: &[HIR],
) {
    for node in body {
        match node {
            HIR::Declare {
                var, expression, ..
            } => {
                if declarations_found.contains(var) {
                    panic!("Variable {} declared more than once", var);
                }
                declarations_found.insert(var.clone());
                check_expr(&declarations_found, function_name, expression);
            }
            HIR::Assign {
                path, expression, ..
            } => {
                if !declarations_found.contains(path.first().unwrap()) {
                    panic!("Assign to undeclared variable {}", path.first().unwrap());
                }
                check_expr(&declarations_found, function_name, expression);
            }
            HIR::FunctionCall { function, args,.. } => {
                check_expr(
                    &declarations_found,
                    function_name,
                    &HIRExpr::Trivial(function.clone(), None),
                );
                for fun_arg in args {
                    check_expr(
                        &declarations_found,
                        function_name,
                        &HIRExpr::Trivial(fun_arg.clone(), None),
                    );
                }
            }
            HIR::Return(expr, ..) => {
                check_expr(&declarations_found, function_name, expr);
            }
            HIR::If(_, true_branch, false_branch, ..) => {
                //we clone the decls so that the scopes are different
                detect_decl_errors_in_body(
                    &mut declarations_found.clone(),
                    function_name,
                    &true_branch,
                );
                detect_decl_errors_in_body(
                    &mut declarations_found.clone(),
                    function_name,
                    &false_branch,
                );
            }

            _ => {}
        };
    }
}

fn detect_declaration_errors_in_function(
    mut declarations_found: HashSet<String>,
    function_name: &str,
    parameters: &[HIRTypedBoundName],
    body: &[HIR],
    return_type: &HIRTypeDef,
) {
    for p in parameters {
        declarations_found.insert(p.name.clone());
    }

    detect_decl_errors_in_body(&mut declarations_found, function_name, body);
}

pub fn detect_undeclared_vars_and_redeclarations(globals: &NameRegistry, mir: &[HIR]) {
    let mut declarations_found = HashSet::<String>::new();

    for name in globals.get_names() {
        declarations_found.insert(name.to_string());
    }

    //first collect all globals
    for node in mir.iter() {
        let result = match node {
            HIR::DeclareFunction { function_name, .. } => {
                declarations_found.insert(function_name.clone());
            }
            _ => {}
        };
    }

    //then check functions
    for node in mir.iter() {
        let result = match node {
            HIR::DeclareFunction {
                function_name,
                parameters,
                body,
                return_type,
                ..
            } => {
                detect_declaration_errors_in_function(
                    declarations_found.clone(),
                    function_name,
                    parameters,
                    body,
                    return_type,
                );
            }
            _ => {}
        };
    }
}
