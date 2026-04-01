use vbash::{Error, ExecError, Shell};

#[test]
fn arith_add() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((2 + 3))").unwrap();
    assert_eq!(r.stdout, "5\n");
}

#[test]
fn arith_sub() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((10 - 4))").unwrap();
    assert_eq!(r.stdout, "6\n");
}

#[test]
fn arith_mul() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((3 * 7))").unwrap();
    assert_eq!(r.stdout, "21\n");
}

#[test]
fn arith_div() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((15 / 4))").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn arith_mod() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((17 % 5))").unwrap();
    assert_eq!(r.stdout, "2\n");
}

#[test]
fn arith_unary_minus() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $(( -(3 + 2) ))").unwrap();
    assert_eq!(r.stdout, "-5\n");
}

#[test]
fn arith_equality() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((5 == 5))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_inequality() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((5 != 3))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_logical_and() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((1 && 0))").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn arith_logical_or() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((0 || 1))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_less_than() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((3 < 5))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_greater_than() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((10 > 7))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_multiply_assign() {
    let mut shell = Shell::new();
    let r = shell.exec("x=4; echo $((x *= 3)); echo $x").unwrap();
    assert_eq!(r.stdout, "12\n12\n");
}

#[test]
fn arith_subtract_assign() {
    let mut shell = Shell::new();
    let r = shell.exec("x=10; echo $((x -= 3)); echo $x").unwrap();
    assert_eq!(r.stdout, "7\n7\n");
}

#[test]
fn arith_divide_assign() {
    let mut shell = Shell::new();
    let r = shell.exec("x=20; echo $((x /= 4)); echo $x").unwrap();
    assert_eq!(r.stdout, "5\n5\n");
}

#[test]
fn arith_ternary() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((5 > 3 ? 1 : 0))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_assignment() {
    let mut shell = Shell::new();
    let r = shell.exec("x=5; echo $((x += 3)); echo $x").unwrap();
    assert_eq!(r.stdout, "8\n8\n");
}

#[test]
fn arith_pre_increment() {
    let mut shell = Shell::new();
    let r = shell.exec("x=5; echo $((++x))").unwrap();
    assert_eq!(r.stdout, "6\n");
}

#[test]
fn arith_post_increment() {
    let mut shell = Shell::new();
    let r = shell.exec("x=5; echo $((x++)); echo $x").unwrap();
    assert_eq!(r.stdout, "5\n6\n");
}

#[test]
fn arith_base_hex() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((16#FF))").unwrap();
    assert_eq!(r.stdout, "255\n");
}

#[test]
fn arith_base_binary() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((2#1010))").unwrap();
    assert_eq!(r.stdout, "10\n");
}

#[test]
fn arith_nested() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $(( (2+3) * (4+1) ))").unwrap();
    assert_eq!(r.stdout, "25\n");
}

#[test]
fn arith_variable() {
    let mut shell = Shell::new();
    let r = shell.exec("x=10; y=20; echo $((x + y))").unwrap();
    assert_eq!(r.stdout, "30\n");
}

#[test]
fn arith_division_by_zero() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((1/0))");
    let err = r.unwrap_err();
    assert!(matches!(err, Error::Exec(ExecError::DivisionByZero)));
}

#[test]
fn arith_negation() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((-5))").unwrap();
    assert_eq!(r.stdout, "-5\n");
}

#[test]
fn arith_logical_not() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((! 0))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_mod_assign() {
    let mut shell = Shell::new();
    let r = shell.exec("x=17; echo $((x %= 5)); echo $x").unwrap();
    assert_eq!(r.stdout, "2\n2\n");
}

#[test]
fn let_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("let x=5+3; echo $x").unwrap();
    assert_eq!(r.stdout, "8\n");
}

#[test]
fn let_exit_code_zero() {
    let mut shell = Shell::new();
    let r = shell.exec("let \"x=0\"; echo $?").unwrap();
    // let returns 1 when the expression evaluates to 0
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn let_exit_code_nonzero() {
    let mut shell = Shell::new();
    let r = shell.exec("let \"x=1\"; echo $?").unwrap();
    // let returns 0 when the expression evaluates to non-zero
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn c_style_for() {
    let mut shell = Shell::new();
    let r = shell.exec("for ((i=0; i<5; i++)); do echo $i; done").unwrap();
    assert_eq!(r.stdout, "0\n1\n2\n3\n4\n");
}

#[test]
fn arith_chained_operations() {
    let mut shell = Shell::new();
    let r = shell.exec("x=2; y=3; echo $((x + y * 4))").unwrap();
    // Multiplication has higher precedence: 2 + (3*4) = 14
    assert_eq!(r.stdout, "14\n");
}

#[test]
fn arith_less_than_or_equal() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((3 <= 3))").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn arith_greater_than_or_equal() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $((5 >= 4))").unwrap();
    assert_eq!(r.stdout, "1\n");
}
