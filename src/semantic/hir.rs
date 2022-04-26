use crate::ast::lexer::*;
use crate::commons::float::*;
use crate::ast::parser::*;

use super::type_db::TypeDatabase;


/**
 * 
 * The HIR expression is not a tree, rather it's a decomposed version of the expression.
 * There is no need to do recursion over a HIR expression, it will be decomposed with more declarations
 * to make type inference easier.
 * 
 * Some of the typechecking is done here, but we might have to lower yet another level
 * to do all the typechecking and other flow control validations, like checking if all paths return a value,
 * and that all returns are compatible
 * 
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrivialHIRExpr {
    IntegerValue(i128),
    FloatValue(Float),
    StringValue(String),
    BooleanValue(bool),
    Variable(String),
    None,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HIRExpr {
    Trivial(TrivialHIRExpr),
    Cast(HIRTypeDef, TrivialHIRExpr),
    BinaryOperation(TrivialHIRExpr, Operator, TrivialHIRExpr),
    FunctionCall(TrivialHIRExpr, Vec<TrivialHIRExpr>),
    UnaryExpression(Operator, TrivialHIRExpr),
    MemberAccess(TrivialHIRExpr, String),
    //maybe the array should have a type hint
    Array(Vec<TrivialHIRExpr>),
}

/*This enum represents the type as typed in source code. This comes from the AST almost directly, 
  no fancy transformations are applied. 
  However we add a Function variant to construct a Function type where needed, but it also could be something coming from the AST in the future, 
  like functions receiving functions*/
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum HIRType {
      Simple(String),
      Generic(String, Vec<HIRType>),
      Function(Vec<HIRType>, Box<HIRType>)
  }

impl HIRExpr {
    fn expect_trivial(&self) -> &TrivialHIRExpr {
        match self {
            HIRExpr::Trivial(e) => e,
            _ => panic!("Expression is not trivial {:?}", self)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HIRTypedBoundName {
    pub name: String,
    pub typename: HIRTypeDef //var name, type
}



#[derive(Debug, Clone, PartialEq, Eq)]
/*Represents a fully resolved type, with generics already substituted */
pub enum TypeInstance {
    Simple(TypeId), //Built-in types, non-generic structs, etc
    Generic(TypeId, Vec<TypeInstance>), //each TypeId in the vec is a type parameter used in this specific usage of the type, this is positional.
    Function(Vec<TypeInstance>, Box<TypeInstance>) //In this case there is not even a base type like in generics, functions are functions 
}

impl TypeInstance {
    pub fn string(&self, type_db: &TypeDatabase) -> String {
        match self {
            TypeInstance::Simple(id) => type_db.get_name(*id).into(),
            TypeInstance::Generic(id, args) => {
                let args_str = args.iter().map(|x| x.string(type_db).clone()).collect::<Vec<_>>().join(", ");
                let base_str =type_db.get_name(*id);
                format!("{}<{}>", base_str, args_str)
            },
            TypeInstance::Function(args, return_type) => {
                let args_str = args.iter().map(|x| x.string(type_db).clone()).collect::<Vec<_>>().join(", ");
                let return_type_str = return_type.string(type_db);
                format!("fn ({}) -> {}", args_str, return_type_str)
            },
        }
    }
}


//we need to be able to represent complex stuff, 
//like a function that receives a function, whose parameters are generic
//def func(another_func: Function<List<String>>)

//so we can store TypeIds, but we need it to be accompanied by more data depending on the kind of the type,
//types such as functions and generics need to be "instanced"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HIRTypeDef {
    Pending,
    Unresolved(HIRType),
    Resolved(TypeInstance)
}

impl HIRTypeDef {
    pub fn expect_unresolved(&self) -> HIRType {
        match self {
            HIRTypeDef::Pending => panic!("Function parameters must have a type"),
            HIRTypeDef::Unresolved(e) => e.clone(),
            HIRTypeDef::Resolved(_) => panic!("Cannot deal with resolved types at this point, this is a bug"),
        }
    }
}

impl HIRType {
    fn from_ast(typ: &ASTType) -> Self {
        match typ {
            ASTType::Simple(name) => Self::Simple(name.clone()),
            ASTType::Generic(name, generics) => {
                let hir_generics = generics.iter().map(|x| Self::from_ast(x)).collect::<Vec<_>>();
                return HIRType::Generic(name.clone(), hir_generics);
            } 
        }
    }
}

pub type TypeId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HIR {
    Assign {
        path: Vec<String>,
        expression: HIRExpr,
    },
    Declare {
        var: String,
        typename: HIRTypeDef,
        expression: HIRExpr,
    },
    DeclareFunction {
        function_name: String,
        parameters: Vec<HIRTypedBoundName>,
        body: Vec<HIR>,
        return_type: HIRTypeDef
    },
    StructDeclaration {
        struct_name: String,
        body: Vec<HIRTypedBoundName>
    },
    FunctionCall {
        function: TrivialHIRExpr,
        args: Vec<TrivialHIRExpr>,
    },
    If(Vec<Expr>, Vec<Expr>),
    Return(HIRExpr),
    EmptyReturn
}


fn make_intermediary(intermediary: i32) -> String {
    return format!("${}", intermediary);
}

//an expression is trivial when it needs basically no effort to 
//check its type. You shouldn't recurse anymore on the expr tree 
fn get_trivial_hir_expr(expr: &Expr) -> Option<TrivialHIRExpr> {
    match expr {
        Expr::IntegerValue(i) => Some(TrivialHIRExpr::IntegerValue(*i)),
        Expr::FloatValue(f) => Some(TrivialHIRExpr::FloatValue(*f)),
        Expr::StringValue(s) => Some(TrivialHIRExpr::StringValue(s.clone())),
        Expr::BooleanValue(b) => Some(TrivialHIRExpr::BooleanValue(*b)),
        Expr::None => Some(TrivialHIRExpr::None),
        Expr::Variable(v) => Some(TrivialHIRExpr::Variable(v.clone())),
        Expr::Parenthesized(_) => panic!("Sanity check: at this point no parenthesized expr should exist"),
        //member accesses are not that simple to prove trivial, let it go through check_if_reducible
       _ => None
    }
}

macro_rules! return_true_if_non_trivial {
    ($e:expr) => {
        let left_is_trivial = get_trivial_hir_expr($e).is_some();
        if !left_is_trivial  {
            return true
        }
    };
}

//If an expression is reducible, you have to call reduce_expr_to_hir_declarations 
//to reduce the expression to a single variable.
fn check_if_reducible(expr: &Expr) -> bool {

    let expr_trivial = get_trivial_hir_expr(expr);
    if expr_trivial.is_some() { return false };


    match expr {
        Expr::FunctionCall(function_call_expr, args_expr) => {
            //a function call is reducible if either side is non-trivial
            //the left side is likely trivial
            return_true_if_non_trivial!(function_call_expr);
            for node in args_expr { return_true_if_non_trivial!(node); }
            return false;
        },
        Expr::BinaryOperation(left, _op, right) => {
            return_true_if_non_trivial!(left);
            return_true_if_non_trivial!(right);
            return false;
        },
        Expr::Array(exprs)  => {
            for e in exprs {
                return_true_if_non_trivial!(e);
            }
            return false;
        },
        Expr::IndexAccess(lhs, index_expr) => {
            //return true so that it can be lowered to a __index__ call
            return true;
        },
        Expr::MemberAccess(path_expr, _member ) => {
            return_true_if_non_trivial!(path_expr);
            return false;
        },
        Expr::UnaryExpression(_operator, expr) => {
            return_true_if_non_trivial!(expr);
            return false;
        }
        _ => true
    }
}

//this function returns the final expression created, and the number of intermediary variables used
//In the recursive cases, this function should always return a HIRExpr::Trivial
fn reduce_expr_to_hir_declarations(expr: &Expr, mut intermediary: i32, accum: &mut Vec<HIR>, is_reducing: bool) -> (HIRExpr, i32) {
    let trivial_expr = get_trivial_hir_expr(expr);
    match trivial_expr {
        Some(x) => { return (HIRExpr::Trivial(x), 0) },
        None => {}
    }

    match expr {
        full_function_call @ Expr::FunctionCall(function_expr, args) => {
           
           let mut total_used_interm = 0;
           
           let fcall = if check_if_reducible(full_function_call) {
             /*
                Either the expr is non-trivial or the args are non-trivial, likely the args are non-trivial
                If an arg is trivial, then it should be just added to the function as-is
                Otherwise (if it's a binary op for instance) then a new variable must be created
                */

                let (lhs_expr, num_interm) = reduce_expr_to_hir_declarations(function_expr, intermediary, accum, true);
                
                intermediary += num_interm;

                let mut args_exprs = vec![];
                let mut args_interm_used = 0;
                for node in args {
                    let (arg_expr, arg_num_interm) = 
                        reduce_expr_to_hir_declarations(node, intermediary, accum, true);
                    intermediary += arg_num_interm;
                    args_interm_used += arg_num_interm;

                   if let HIRExpr::Trivial(arg) = arg_expr {
                        args_exprs.push(arg);
                    } else {
                        panic!("Function call expression: after reduction, argument should be trivial!");
                    };
                }

                total_used_interm = num_interm + args_interm_used;
                let call_expr = if let HIRExpr::Trivial(name) = lhs_expr {
                    name
                } else {
                    panic!("Function call expression: should be bound to a name!")
                };

                HIRExpr::FunctionCall(call_expr, args_exprs)
           } else {
                let args = args.iter().map(|x| get_trivial_hir_expr(x).unwrap()).collect::<Vec<_>>();
                HIRExpr::FunctionCall(get_trivial_hir_expr(function_expr).unwrap(), args)
           };

           if is_reducing {
                let declare = HIR::Declare {
                    var: make_intermediary(intermediary),
                    typename: HIRTypeDef::Pending,
                    expression: fcall.clone()
                };
                total_used_interm += 1;
                accum.push(declare);
                return (HIRExpr::Trivial(TrivialHIRExpr::Variable(make_intermediary(intermediary))), total_used_interm);
            } else {
                return (fcall, total_used_interm);
            }
           
        }
        full_binop @ Expr::BinaryOperation(lhs, op, rhs) => {
            let mut total_used_interm = 0;
            let binop = if check_if_reducible(full_binop) {
                let (lhs_intermediary, lhs_num_intern) = reduce_expr_to_hir_declarations(lhs, intermediary, accum, true);
                intermediary += lhs_num_intern;

                let (rhs_intermediary, rhs_num_intern) = reduce_expr_to_hir_declarations(rhs, intermediary, accum, true);
                intermediary += rhs_num_intern;
                
                total_used_interm = lhs_num_intern + rhs_num_intern;

                HIRExpr::BinaryOperation(
                    lhs_intermediary.expect_trivial().clone(), 
                    *op, 
                    rhs_intermediary.expect_trivial().clone())
            } else {
                HIRExpr::BinaryOperation(
                    get_trivial_hir_expr(lhs).unwrap(),
                    *op,
                    get_trivial_hir_expr(rhs).unwrap()
                )
            };
                
            if is_reducing {
                let declare = HIR::Declare {
                    var: make_intermediary(intermediary),
                    typename: HIRTypeDef::Pending,
                    expression: binop.clone()
                };
                total_used_interm += 1;
                accum.push(declare);

                return (HIRExpr::Trivial(
                    TrivialHIRExpr::Variable(make_intermediary(intermediary))), total_used_interm);
            } else {
                return (binop, total_used_interm);
            }
        },
        Expr::Variable(var) => {
            return (HIRExpr::Trivial(TrivialHIRExpr::Variable(var.clone())), 0);
        },
        full_array_exp @ Expr::Array(arr_exprs) => {

            let mut total_used_interm = 0;

            let array = if check_if_reducible(full_array_exp) {
                let mut item_exprs = vec![];
                for node in arr_exprs {
                    let (item_expr, item_num_interm) = 
                        reduce_expr_to_hir_declarations(node, intermediary, accum, true);
                    intermediary += item_num_interm;
                    total_used_interm += item_num_interm;

                    if let HIRExpr::Trivial(arg) = item_expr {
                        item_exprs.push(arg);
                    } else {
                        panic!("Array expression item: after reduction, argument should be trivial!");
                    };
                }

                HIRExpr::Array(item_exprs)
            } else {
                let args = arr_exprs.iter().map(|x| get_trivial_hir_expr(x).unwrap()).collect::<Vec<_>>();
                HIRExpr::Array(args)
            };

            if is_reducing {
                let declare = HIR::Declare {
                    var: make_intermediary(intermediary),
                    typename: HIRTypeDef::Pending,
                    expression: array.clone()
                };
                total_used_interm += 1;
                accum.push(declare);
                return (HIRExpr::Trivial(TrivialHIRExpr::Variable(make_intermediary(intermediary))), total_used_interm);
            } else {
                return (array, total_used_interm);
            }
        },
        //transforms an index access into a method call on obj
        //i.e. if obj[0], becomes obj.__index__(0)
        //i.e. if obj.map[0] becomes obj.map.__index__(0)
        //will need member access syntax support
        Expr::IndexAccess(obj_expr, index_expr) => {
            let owned = index_expr.to_owned();
            let as_fcall = Expr::FunctionCall(
                Box::new(Expr::MemberAccess(obj_expr.clone(), "__index__".into())),
                vec![*owned]
            );

            return reduce_expr_to_hir_declarations(&as_fcall, intermediary, accum, is_reducing);
        },
        unary_expression @ Expr::UnaryExpression(op, expr) => {
            let mut total_used_interm = 0;
            let unaryop = if check_if_reducible(unary_expression) {
                let (expr_intermediary, num_intern) = reduce_expr_to_hir_declarations(expr, intermediary, accum, true);
                intermediary += num_intern;

                total_used_interm = num_intern;

                HIRExpr::UnaryExpression(
                    *op, 
                    expr_intermediary.expect_trivial().clone())
            } else {
                HIRExpr::UnaryExpression(
                    *op,
                    get_trivial_hir_expr(expr).unwrap()
                )
            };
                
            if is_reducing {
                let declare = HIR::Declare {
                    var: make_intermediary(intermediary),
                    typename: HIRTypeDef::Pending,
                    expression: unaryop.clone()
                };
                total_used_interm += 1;
                accum.push(declare);

                return (HIRExpr::Trivial(
                    TrivialHIRExpr::Variable(make_intermediary(intermediary))), total_used_interm);
            } else {
                return (unaryop, total_used_interm);
            }
        }
        Expr::MemberAccess(obj_expr, name) => {
            let mut total_used_interm = 0;
            let member_access = if check_if_reducible(obj_expr) {
                let (expr_intermediary, num_intern) = reduce_expr_to_hir_declarations(obj_expr, intermediary, accum, true);
                intermediary += num_intern;

                total_used_interm = num_intern;

                HIRExpr::MemberAccess(
                    expr_intermediary.expect_trivial().clone(), 
                    name.clone())
            } else {
                HIRExpr::MemberAccess(
                    get_trivial_hir_expr(obj_expr).unwrap(), 
                    name.clone()
                )
            };

            if is_reducing {
                let declare = HIR::Declare {
                    var: make_intermediary(intermediary),
                    typename: HIRTypeDef::Pending,
                    expression: member_access.clone()
                };
                total_used_interm += 1;
                accum.push(declare);

                return (HIRExpr::Trivial(
                    TrivialHIRExpr::Variable(make_intermediary(intermediary))), total_used_interm);
            } else {
                return (member_access, total_used_interm);
            }
        }
        exprnode => panic!("Expr to HIR not implemented for {:?}", exprnode)
    }
}

pub fn ast_to_hir(ast: &AST, mut intermediary: i32, accum: &mut Vec<HIR>) -> i32 {

    match ast {
        AST::Declare {var, expression} => {

            //expr: we have to decompose the expression into HIR declarations
            //because the assigned/declared expression has to be sufficiently simple
            //to decrease complexity for the other compiler phases (typechecking, type inference)

            //maybe a way to do it is by calling reduce_expr_to_hir_declarations, and the function
            //itself returns a HIRExpr. It will also add to the HIR any declarations needed
            //for the decomposition.
            let (result_expr, num_intermediaries) = reduce_expr_to_hir_declarations(expression, intermediary, accum, false);
            
            let decl_hir = HIR::Declare {
                var: var.name.clone(),
                typename: HIRTypeDef::Unresolved(HIRType::from_ast(&var.name_type)),
                expression: result_expr
            };

            accum.push(decl_hir);

            return num_intermediaries;
        },
        AST::Assign {path, expression} => {
            let (result_expr, num_intermediaries) = reduce_expr_to_hir_declarations(expression, intermediary, accum, false);
            
            let decl_hir = HIR::Assign {
                path: path.clone(),
                expression: result_expr
            };

            accum.push(decl_hir);
            return num_intermediaries;
        },
        AST::DeclareFunction { function_name, parameters, body, return_type} => {

            let mut function_body = vec![];

            for node in body {
                let created_intermediaries = ast_to_hir(node, intermediary, &mut function_body);
                intermediary += created_intermediaries;
            }

            let decl_hir = HIR::DeclareFunction {
                function_name: function_name.clone(),
                parameters: parameters.iter().map(|param| {
                    let name = param.name.clone();
                    return HIRTypedBoundName {
                        name, typename: HIRTypeDef::Unresolved(HIRType::from_ast(&param.name_type))
                    }
                }).collect(),
                body: function_body,
                return_type: match return_type {
                    Some(x) => HIRTypeDef::Unresolved(HIRType::from_ast(x)),
                    None => HIRTypeDef::Unresolved(HIRType::Simple("Void".into()))
                }
            };

            accum.push(decl_hir);
            return 0; //yes, the function decls themselves created intermediaries, but they don't 
            //escape the context
        }
        AST::Root(ast_nodes) => {
            let mut sum_intermediaries = 0;
            for node in ast_nodes {
                let created_intermediaries = ast_to_hir(node, intermediary, accum);
                sum_intermediaries += created_intermediaries;
                intermediary += created_intermediaries;
            }
            return sum_intermediaries;
        }
        AST::Return(expr) => {
            match expr {
                None => {
                    accum.push(HIR::EmptyReturn);
                    return 0;
                },
                Some(e) => {
                    let (result_expr, num_intermediaries) = reduce_expr_to_hir_declarations(e, intermediary, accum, false);
                    accum.push(HIR::Return(result_expr));
                    return num_intermediaries;
                }
            }
        }
        AST::StructDeclaration {struct_name, body} => {
            let fields = body.iter().map(|field| {
                return HIRTypedBoundName { 
                    name: field.name.clone(), 
                    typename: HIRTypeDef::Unresolved(HIRType::from_ast(&field.name_type)) };
            });
            accum.push(HIR::StructDeclaration{ struct_name: struct_name.clone(), body: fields.collect()});
            return 0;
        }
        AST::StandaloneExpr(expr) => {

            let Expr::FunctionCall(_, _) = expr else {
                panic!("Can only lower function call standalone expr");
            };

            let (result_expr, num_intermediaries) = reduce_expr_to_hir_declarations(expr, intermediary, accum, false);
            let HIRExpr::FunctionCall(function, args) = &result_expr else {
                panic!("Lowering of function call returned invalid result: {:?}", result_expr);
            };
            accum.push(HIR::FunctionCall {function: function.clone(), args: args.clone()});
            return num_intermediaries;
        }
        ast => panic!("Not implemented HIR for {:?}", ast)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::*;
    use crate::ast::parser::*;
    use crate::ast::lexer::*;

    //Parses a single expression
    fn parse(source: &str) -> Vec<HIR> {
        let tokens = crate::ast::lexer::tokenize(source);
        //println!("Tokens: {:?}", tokens);
        let ast = crate::ast::parser::parse_ast(tokens.unwrap());

        let root = crate::ast::parser::AST::Root(ast);
        let mut result = vec![];
        hir::ast_to_hir(&root, 0, &mut result);
        return result;
    }

    #[test]
    fn hir_multiline_code() {
        let result = parse(
            "
x = 'abc' + 'cde'
y = x + str(True)",
        );
        
        let expected = vec![
            HIR::Assign { 
                path: vec!["x".into()], 
                expression: HIRExpr::BinaryOperation(
                    TrivialHIRExpr::StringValue("abc".into()), 
                    Operator::Plus, 
                    TrivialHIRExpr::StringValue("cde".into())) 
            }, 
            HIR::Declare { 
                var: "$0".into(), 
                typename: HIRTypeDef::Pending, 
                expression: HIRExpr::FunctionCall(
                    TrivialHIRExpr::Variable("str".into()), 
                    vec![TrivialHIRExpr::BooleanValue(true)]) 
            }, 
            HIR::Assign { 
                path: vec!["y".into()], 
                expression: HIRExpr::BinaryOperation(
                    TrivialHIRExpr::Variable("x".into()), 
                    Operator::Plus, 
                    TrivialHIRExpr::Variable("$0".into())) 
            }
        ];

        assert_eq!(expected, result);
    }


    #[test]
    fn complex_code() {
        let result = parse(
            "
def main(args: List<String>):
    minus: i32 = -my_function(99, 999)
    numbers = [1,2, -3, minus]
    r1 = my_function(1, 2)
    r2 = my_function2(3, 4)
    r3 = my_function(numbers[1], numbers[2])
    print(r1 + r2 + r3)

def my_function(arg1: i32, arg2: i32) -> i32:
    return arg1 * arg2 / (arg2 - arg1)

def my_function2(arg1: i32, arg2: i32) -> i32:
    result1: i32 = my_function(arg1, arg2 + 1)
    result2 = pow(arg1, arg2 * 9)
    return my_function(result1, result2)
",
        );
        
        let debug_view_expected = "[DeclareFunction { function_name: \"main\", parameters: [HIRTypedBoundName { name: \"args\", typename: Unresolved(Generic(\"List\", [Unresolved(Simple(\"String\"))])) }], body: [Declare { var: \"$0\", typename: Pending, expression: FunctionCall(Variable(\"my_function\"), [IntegerValue(99), IntegerValue(999)]) }, Declare { var: \"minus\", typename: Unresolved(Simple(\"i32\")), expression: UnaryExpression(Minus, Variable(\"$0\")) }, Declare { var: \"$1\", typename: Pending, expression: UnaryExpression(Minus, IntegerValue(3)) }, Assign { path: [\"numbers\"], expression: Array([IntegerValue(1), IntegerValue(2), Variable(\"$1\"), Variable(\"minus\")]) }, Assign { path: [\"r1\"], expression: FunctionCall(Variable(\"my_function\"), [IntegerValue(1), IntegerValue(2)]) }, Assign { path: [\"r2\"], expression: FunctionCall(Variable(\"my_function2\"), [IntegerValue(3), IntegerValue(4)]) }, Declare { var: \"$2\", typename: Pending, expression: IndexAccess(Variable(\"numbers\"), IntegerValue(1)) }, Declare { var: \"$3\", typename: Pending, expression: IndexAccess(Variable(\"numbers\"), IntegerValue(2)) }, Assign { path: [\"r3\"], expression: FunctionCall(Variable(\"my_function\"), [Variable(\"$2\"), Variable(\"$3\")]) }, Declare { var: \"$4\", typename: Pending, expression: BinaryOperation(Variable(\"r1\"), Plus, Variable(\"r2\")) }, Declare { var: \"$5\", typename: Pending, expression: BinaryOperation(Variable(\"$4\"), Plus, Variable(\"r3\")) }, FunctionCall { function: Variable(\"print\"), args: [Variable(\"$5\")] }], return_type: Unresolved(Simple(\"Void\")) }, DeclareFunction { function_name: \"my_function\", parameters: [HIRTypedBoundName { name: \"arg1\", typename: Unresolved(Simple(\"i32\")) }, HIRTypedBoundName { name: \"arg2\", typename: Unresolved(Simple(\"i32\")) }], body: [Declare { var: \"$0\", typename: Pending, expression: BinaryOperation(Variable(\"arg1\"), Multiply, Variable(\"arg2\")) }, Declare { var: \"$1\", typename: Pending, expression: BinaryOperation(Variable(\"arg2\"), Minus, Variable(\"arg1\")) }, Return(BinaryOperation(Variable(\"$0\"), Divide, Variable(\"$1\")))], return_type: Unresolved(Simple(\"i32\")) }, DeclareFunction { function_name: \"my_function2\", parameters: [HIRTypedBoundName { name: \"arg1\", typename: Unresolved(Simple(\"i32\")) }, HIRTypedBoundName { name: \"arg2\", typename: Unresolved(Simple(\"i32\")) }], body: [Declare { var: \"$0\", typename: Pending, expression: BinaryOperation(Variable(\"arg2\"), Plus, IntegerValue(1)) }, Declare { var: \"result1\", typename: Unresolved(Simple(\"i32\")), expression: FunctionCall(Variable(\"my_function\"), [Variable(\"arg1\"), Variable(\"$0\")]) }, Declare { var: \"$1\", typename: Pending, expression: BinaryOperation(Variable(\"arg2\"), Multiply, IntegerValue(9)) }, Assign { path: [\"result2\"], expression: FunctionCall(Variable(\"pow\"), [Variable(\"arg1\"), Variable(\"$1\")]) }, Return(FunctionCall(Variable(\"my_function\"), [Variable(\"result1\"), Variable(\"result2\")]))], return_type: Unresolved(Simple(\"i32\")) }]";

        assert_eq!(debug_view_expected, format!("{:?}", result));
    }

    
}