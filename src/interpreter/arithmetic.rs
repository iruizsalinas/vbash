use crate::ast::ArithExpr;
use crate::error::{ExecError, ShellSignal};

use super::Interpreter;

impl Interpreter<'_> {
    pub(super) fn evaluate_arith(&mut self, expr: &ArithExpr) -> Result<i64, ShellSignal> {
        match expr {
            ArithExpr::Number(n) => Ok(*n),
            ArithExpr::Variable(name) => {
                // If it looks like an expression (contains operators/spaces),
                // evaluate it as a runtime arithmetic string
                if name.contains('+') || name.contains('-') || name.contains('*')
                    || name.contains('/') || name.contains('%') || name.contains(' ')
                    || name.contains('<') || name.contains('>') || name.contains('=')
                    || name.contains('#')
                {
                    return self.evaluate_arith_string(name);
                }
                let val = self.state.get_var(name).unwrap_or("0");
                Ok(val.parse::<i64>().unwrap_or(0))
            }
            ArithExpr::Binary { op, left, right } => {
                use crate::ast::arithmetic::ArithBinaryOp;
                let l = self.evaluate_arith(left)?;
                let r = self.evaluate_arith(right)?;
                match op {
                    ArithBinaryOp::Add => Ok(l.wrapping_add(r)),
                    ArithBinaryOp::Sub => Ok(l.wrapping_sub(r)),
                    ArithBinaryOp::Mul => Ok(l.wrapping_mul(r)),
                    ArithBinaryOp::Div => {
                        if r == 0 {
                            Err(ExecError::DivisionByZero.into())
                        } else {
                            Ok(l / r)
                        }
                    }
                    ArithBinaryOp::Mod => {
                        if r == 0 {
                            Err(ExecError::DivisionByZero.into())
                        } else {
                            Ok(l % r)
                        }
                    }
                    ArithBinaryOp::Pow => Ok(l.wrapping_pow(u32::try_from(r.clamp(0, i64::from(u32::MAX))).unwrap_or(u32::MAX))),
                    ArithBinaryOp::ShiftLeft => Ok(l << (r & 63)),
                    ArithBinaryOp::ShiftRight => Ok(l >> (r & 63)),
                    ArithBinaryOp::Lt => Ok(i64::from(l < r)),
                    ArithBinaryOp::Le => Ok(i64::from(l <= r)),
                    ArithBinaryOp::Gt => Ok(i64::from(l > r)),
                    ArithBinaryOp::Ge => Ok(i64::from(l >= r)),
                    ArithBinaryOp::Eq => Ok(i64::from(l == r)),
                    ArithBinaryOp::Ne => Ok(i64::from(l != r)),
                    ArithBinaryOp::BitAnd => Ok(l & r),
                    ArithBinaryOp::BitOr => Ok(l | r),
                    ArithBinaryOp::BitXor => Ok(l ^ r),
                    ArithBinaryOp::LogicalAnd => Ok(i64::from(l != 0 && r != 0)),
                    ArithBinaryOp::LogicalOr => Ok(i64::from(l != 0 || r != 0)),
                }
            }
            ArithExpr::Unary { op, operand, prefix } => {
                use crate::ast::arithmetic::ArithUnaryOp;
                let val = self.evaluate_arith(operand)?;
                match op {
                    ArithUnaryOp::Negate => Ok(-val),
                    ArithUnaryOp::Plus => Ok(val),
                    ArithUnaryOp::LogicalNot => Ok(i64::from(val == 0)),
                    ArithUnaryOp::BitNot => Ok(!val),
                    ArithUnaryOp::Increment => {
                        let new_val = val + 1;
                        if let ArithExpr::Variable(name) = operand.as_ref() {
                            let _ = self.state.set_var(name, new_val.to_string());
                        }
                        if *prefix { Ok(new_val) } else { Ok(val) }
                    }
                    ArithUnaryOp::Decrement => {
                        let new_val = val - 1;
                        if let ArithExpr::Variable(name) = operand.as_ref() {
                            let _ = self.state.set_var(name, new_val.to_string());
                        }
                        if *prefix { Ok(new_val) } else { Ok(val) }
                    }
                }
            }
            ArithExpr::Ternary { condition, consequent, alternate } => {
                let cond = self.evaluate_arith(condition)?;
                if cond != 0 {
                    self.evaluate_arith(consequent)
                } else {
                    self.evaluate_arith(alternate)
                }
            }
            ArithExpr::Group(inner) => self.evaluate_arith(inner),
            ArithExpr::Comma { left, right } => {
                self.evaluate_arith(left)?;
                self.evaluate_arith(right)
            }
            ArithExpr::Assign { name, op, value, .. } => {
                use crate::ast::arithmetic::ArithAssignOp;
                let rhs = self.evaluate_arith(value)?;
                let new_val = if *op == ArithAssignOp::Assign {
                    rhs
                } else {
                    let old = self.state.get_var(name).unwrap_or("0").parse::<i64>().unwrap_or(0);
                    match op {
                        ArithAssignOp::AddAssign => old.wrapping_add(rhs),
                        ArithAssignOp::SubAssign => old.wrapping_sub(rhs),
                        ArithAssignOp::MulAssign => old.wrapping_mul(rhs),
                        ArithAssignOp::DivAssign if rhs != 0 => old / rhs,
                        ArithAssignOp::ModAssign if rhs != 0 => old % rhs,
                        ArithAssignOp::ShiftLeftAssign => old << (rhs & 63),
                        ArithAssignOp::ShiftRightAssign => old >> (rhs & 63),
                        ArithAssignOp::BitAndAssign => old & rhs,
                        ArithAssignOp::BitOrAssign => old | rhs,
                        ArithAssignOp::BitXorAssign => old ^ rhs,
                        ArithAssignOp::DivAssign | ArithAssignOp::ModAssign => {
                            return Err(ExecError::DivisionByZero.into());
                        }
                        ArithAssignOp::Assign => unreachable!(),
                    }
                };
                let _ = self.state.set_var(name, new_val.to_string());
                Ok(new_val)
            }
            ArithExpr::Nested(inner) => self.evaluate_arith(inner),
            ArithExpr::ArrayElement { .. }
            | ArithExpr::CommandSubst(_)
            | ArithExpr::ParameterExpansion(_) => Ok(0),
        }
    }

    /// Evaluate a runtime arithmetic string like "i + 1" or "x * 2 + 3".
    /// Supports variables (looked up in shell state), +, -, *, /, %, comparisons.
    pub(super) fn evaluate_arith_string(&mut self, expr: &str) -> Result<i64, ShellSignal> {
        self.evaluate_arith_string_depth(expr, 0)
    }

    fn evaluate_arith_string_depth(&mut self, expr: &str, depth: u32) -> Result<i64, ShellSignal> {
        const MAX_ARITH_DEPTH: u32 = 200;
        if depth > MAX_ARITH_DEPTH {
            return Err(ExecError::Other("arithmetic expression recursion limit exceeded".to_string()).into());
        }
        let expr = expr.trim();
        if expr.is_empty() {
            return Ok(0);
        }

        if let Some((lhs, op, rhs)) = split_arith_assign(expr) {
            let rhs_val = self.evaluate_arith_string_depth(rhs, depth + 1)?;
            let new_val = if op.is_empty() {
                rhs_val
            } else {
                let old_val = self.state.get_var(lhs.trim()).unwrap_or("0").parse::<i64>().unwrap_or(0);
                match op {
                    "+" => old_val.wrapping_add(rhs_val),
                    "-" => old_val.wrapping_sub(rhs_val),
                    "*" => old_val.wrapping_mul(rhs_val),
                    "/" if rhs_val != 0 => old_val / rhs_val,
                    "%" if rhs_val != 0 => old_val % rhs_val,
                    _ => rhs_val,
                }
            };
            let _ = self.state.set_var(lhs.trim(), new_val.to_string());
            return Ok(new_val);
        }

        if let Some((cond, a, b)) = split_ternary(expr) {
            let c = self.evaluate_arith_string_depth(cond, depth + 1)?;
            return if c != 0 {
                self.evaluate_arith_string_depth(a, depth + 1)
            } else {
                self.evaluate_arith_string_depth(b, depth + 1)
            };
        }

        for &(op_str, op_fn) in &[
            ("||", arith_op_lor as fn(i64, i64) -> i64),
            ("&&", arith_op_land),
        ] {
            if let Some((l, r)) = split_binary_op(expr, op_str) {
                let lv = self.evaluate_arith_string_depth(l, depth + 1)?;
                let rv = self.evaluate_arith_string_depth(r, depth + 1)?;
                return Ok(op_fn(lv, rv));
            }
        }

        // Bitwise OR: | but not ||
        {
            let mut paren_depth = 0i32;
            let bytes = expr.as_bytes();
            let mut split_pos = None;
            for i in (0..bytes.len()).rev() {
                match bytes[i] {
                    b')' => paren_depth += 1,
                    b'(' => paren_depth -= 1,
                    b'|' if paren_depth == 0 && i > 0 => {
                        let prev_is_pipe = i > 0 && bytes[i - 1] == b'|';
                        let next_is_pipe = i + 1 < bytes.len() && bytes[i + 1] == b'|';
                        if !prev_is_pipe && !next_is_pipe {
                            split_pos = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(pos) = split_pos {
                let lv = self.evaluate_arith_string_depth(&expr[..pos], depth + 1)?;
                let rv = self.evaluate_arith_string_depth(&expr[pos + 1..], depth + 1)?;
                return Ok(lv | rv);
            }
        }

        // Bitwise XOR: ^
        if let Some((l, r)) = split_binary_op(expr, "^") {
            let lv = self.evaluate_arith_string_depth(l, depth + 1)?;
            let rv = self.evaluate_arith_string_depth(r, depth + 1)?;
            return Ok(lv ^ rv);
        }

        // Bitwise AND: & but not &&
        {
            let mut paren_depth = 0i32;
            let bytes = expr.as_bytes();
            let mut split_pos = None;
            for i in (0..bytes.len()).rev() {
                match bytes[i] {
                    b')' => paren_depth += 1,
                    b'(' => paren_depth -= 1,
                    b'&' if paren_depth == 0 && i > 0 => {
                        let prev_is_amp = i > 0 && bytes[i - 1] == b'&';
                        let next_is_amp = i + 1 < bytes.len() && bytes[i + 1] == b'&';
                        if !prev_is_amp && !next_is_amp {
                            split_pos = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(pos) = split_pos {
                let lv = self.evaluate_arith_string_depth(&expr[..pos], depth + 1)?;
                let rv = self.evaluate_arith_string_depth(&expr[pos + 1..], depth + 1)?;
                return Ok(lv & rv);
            }
        }

        for &(op_str, op_fn) in &[
            ("==", arith_op_eq as fn(i64, i64) -> i64),
            ("!=", arith_op_ne),
            ("<=", arith_op_le),
            (">=", arith_op_ge),
        ] {
            if let Some((l, r)) = split_binary_op(expr, op_str) {
                let lv = self.evaluate_arith_string_depth(l, depth + 1)?;
                let rv = self.evaluate_arith_string_depth(r, depth + 1)?;
                return Ok(op_fn(lv, rv));
            }
        }

        // Bitwise shifts: << and >> (must be checked before single < and >)
        for &(op_str, op_fn) in &[
            ("<<", arith_op_shl as fn(i64, i64) -> i64),
            (">>", arith_op_shr),
        ] {
            if let Some((l, r)) = split_binary_op(expr, op_str) {
                let lv = self.evaluate_arith_string_depth(l, depth + 1)?;
                let rv = self.evaluate_arith_string_depth(r, depth + 1)?;
                return Ok(op_fn(lv, rv));
            }
        }

        // Single < and > comparisons: must not match << or >>
        {
            let mut paren_depth = 0i32;
            let bytes = expr.as_bytes();
            let mut split_pos = None;
            for i in (0..bytes.len()).rev() {
                match bytes[i] {
                    b')' => paren_depth += 1,
                    b'(' => paren_depth -= 1,
                    b'<' | b'>' if paren_depth == 0 && i > 0 => {
                        let ch = bytes[i];
                        let prev_same = i > 0 && bytes[i - 1] == ch;
                        let next_same = i + 1 < bytes.len() && bytes[i + 1] == ch;
                        let next_eq = i + 1 < bytes.len() && bytes[i + 1] == b'=';
                        if !prev_same && !next_same && !next_eq {
                            split_pos = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(pos) = split_pos {
                let lv = self.evaluate_arith_string_depth(&expr[..pos], depth + 1)?;
                let rv = self.evaluate_arith_string_depth(&expr[pos + 1..], depth + 1)?;
                return Ok(if bytes[pos] == b'<' { arith_op_lt(lv, rv) } else { arith_op_gt(lv, rv) });
            }
        }

        // Find rightmost +/- not inside parens (left-to-right associativity)
        {
            let mut paren_depth = 0i32;
            let bytes = expr.as_bytes();
            let mut split_pos = None;
            for i in (0..bytes.len()).rev() {
                match bytes[i] {
                    b')' => paren_depth += 1,
                    b'(' => paren_depth -= 1,
                    b'+' | b'-' if paren_depth == 0 && i > 0 => {
                        let next_is_same = i + 1 < bytes.len() && bytes[i + 1] == bytes[i];
                        if !next_is_same && !matches!(bytes[i - 1], b'+' | b'-' | b'<' | b'>' | b'=' | b'!' | b'*' | b'/') {
                            split_pos = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(pos) = split_pos {
                let lv = self.evaluate_arith_string_depth(&expr[..pos], depth + 1)?;
                let op = bytes[pos];
                let rv = self.evaluate_arith_string_depth(&expr[pos + 1..], depth + 1)?;
                return Ok(if op == b'+' { lv.wrapping_add(rv) } else { lv.wrapping_sub(rv) });
            }
        }

        // Power operator ** (right-associative, higher precedence than */%)
        {
            let bytes = expr.as_bytes();
            let mut paren_depth = 0i32;
            for i in 0..bytes.len().saturating_sub(1) {
                match bytes[i] {
                    b'(' => paren_depth += 1,
                    b')' => paren_depth -= 1,
                    b'*' if paren_depth == 0 && bytes[i + 1] == b'*' => {
                        let lv = self.evaluate_arith_string_depth(&expr[..i], depth + 1)?;
                        let rv = self.evaluate_arith_string_depth(&expr[i + 2..], depth + 1)?;
                        return Ok(lv.wrapping_pow(u32::try_from(rv.clamp(0, i64::from(u32::MAX))).unwrap_or(u32::MAX)));
                    }
                    _ => {}
                }
            }
        }

        // Multiplication, division, modulo
        {
            let mut paren_depth = 0i32;
            let bytes = expr.as_bytes();
            let mut split_pos = None;
            for i in (0..bytes.len()).rev() {
                match bytes[i] {
                    b')' => paren_depth += 1,
                    b'(' => paren_depth -= 1,
                    b'*' | b'/' | b'%' if paren_depth == 0 => {
                        if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                            continue;
                        }
                        split_pos = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
            if let Some(pos) = split_pos {
                let lv = self.evaluate_arith_string_depth(&expr[..pos], depth + 1)?;
                let op = bytes[pos];
                let rv = self.evaluate_arith_string_depth(&expr[pos + 1..], depth + 1)?;
                return match op {
                    b'*' => Ok(lv.wrapping_mul(rv)),
                    b'/' => {
                        if rv == 0 { Err(ExecError::DivisionByZero.into()) }
                        else { Ok(lv / rv) }
                    }
                    b'%' => {
                        if rv == 0 { Err(ExecError::DivisionByZero.into()) }
                        else { Ok(lv % rv) }
                    }
                    _ => Ok(0),
                };
            }
        }

        let trimmed = expr.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            return self.evaluate_arith_string_depth(&trimmed[1..trimmed.len() - 1], depth + 1);
        }

        if let Some(rest) = trimmed.strip_prefix('-') {
            let val = self.evaluate_arith_string_depth(rest, depth + 1)?;
            return Ok(-val);
        }

        if let Some(rest) = trimmed.strip_prefix('!') {
            let val = self.evaluate_arith_string_depth(rest, depth + 1)?;
            return Ok(i64::from(val == 0));
        }

        if let Some(var) = trimmed.strip_suffix("++") {
            let var = var.trim();
            let val = self.state.get_var(var).unwrap_or("0").parse::<i64>().unwrap_or(0);
            let _ = self.state.set_var(var, (val + 1).to_string());
            return Ok(val);
        }
        if let Some(var) = trimmed.strip_prefix("++") {
            let var = var.trim();
            let val = self.state.get_var(var).unwrap_or("0").parse::<i64>().unwrap_or(0) + 1;
            let _ = self.state.set_var(var, val.to_string());
            return Ok(val);
        }

        if let Some(hash_pos) = trimmed.find('#') {
            let base_str = &trimmed[..hash_pos];
            let value_str = &trimmed[hash_pos + 1..];
            if let Ok(base) = base_str.parse::<u32>() {
                if (2..=64).contains(&base) {
                    if let Ok(val) = i64::from_str_radix(value_str, base) {
                        return Ok(val);
                    }
                }
            }
        }

        if let Ok(n) = trimmed.parse::<i64>() {
            return Ok(n);
        }

        let var_name = trimmed.strip_prefix('$').unwrap_or(trimmed);
        let val = self.resolve_arith_var(var_name);
        Ok(val.parse::<i64>().unwrap_or(0))
    }

    /// Resolve a variable name in arithmetic context, handling positional
    /// parameters ($1, $2, ...) and special variables ($#, $?, etc.).
    fn resolve_arith_var(&self, name: &str) -> String {
        // Single digit: positional parameter
        if name.len() == 1 && name.as_bytes()[0].is_ascii_digit() {
            let idx = (name.as_bytes()[0] - b'0') as usize;
            if idx == 0 {
                return "0".to_string();
            }
            return self.state.positional_params
                .get(idx - 1)
                .cloned()
                .unwrap_or_else(|| "0".to_string());
        }
        // Multi-digit positional parameter
        if let Ok(idx) = name.parse::<usize>() {
            if idx == 0 {
                return "0".to_string();
            }
            return self.state.positional_params
                .get(idx - 1)
                .cloned()
                .unwrap_or_else(|| "0".to_string());
        }
        match name {
            "#" => self.state.positional_params.len().to_string(),
            "?" => self.state.last_exit_code.to_string(),
            _ => self.state.get_var(name).unwrap_or("0").to_string(),
        }
    }
}

pub(super) fn arith_op_lor(a: i64, b: i64) -> i64 { i64::from(a != 0 || b != 0) }
pub(super) fn arith_op_land(a: i64, b: i64) -> i64 { i64::from(a != 0 && b != 0) }
pub(super) fn arith_op_eq(a: i64, b: i64) -> i64 { i64::from(a == b) }
pub(super) fn arith_op_ne(a: i64, b: i64) -> i64 { i64::from(a != b) }
pub(super) fn arith_op_lt(a: i64, b: i64) -> i64 { i64::from(a < b) }
pub(super) fn arith_op_le(a: i64, b: i64) -> i64 { i64::from(a <= b) }
pub(super) fn arith_op_gt(a: i64, b: i64) -> i64 { i64::from(a > b) }
pub(super) fn arith_op_ge(a: i64, b: i64) -> i64 { i64::from(a >= b) }
fn arith_op_shl(a: i64, b: i64) -> i64 { a.wrapping_shl((b & 63) as u32) }
fn arith_op_shr(a: i64, b: i64) -> i64 { a.wrapping_shr((b & 63) as u32) }

/// Split `var = expr` or `var += expr` assignments.
/// Returns `(var_name, op, rhs)` where op is "", "+", "-", "*", "/", or "%".
pub(super) fn split_arith_assign(expr: &str) -> Option<(&str, &str, &str)> {
    let bytes = expr.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'=' && (i + 1 >= bytes.len() || bytes[i + 1] != b'=') {
            let prev = bytes[i - 1];
            if prev == b'!' || prev == b'<' || prev == b'>' || prev == b'=' {
                continue;
            }
            let (lhs, op) = if prev == b'+' || prev == b'-' || prev == b'*' || prev == b'/' || prev == b'%' {
                (&expr[..i - 1], &expr[i - 1..i])
            } else {
                (&expr[..i], "")
            };
            if lhs.trim().bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && !lhs.trim().is_empty() {
                return Some((lhs, op, &expr[i + 1..]));
            }
        }
    }
    None
}

/// Split `cond ? a : b` ternary.
pub(super) fn split_ternary(expr: &str) -> Option<(&str, &str, &str)> {
    let bytes = expr.as_bytes();
    let mut depth = 0i32;
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'?' if depth == 0 => {
                let mut d2 = 0i32;
                for j in (i + 1)..bytes.len() {
                    match bytes[j] {
                        b'(' => d2 += 1,
                        b')' => d2 -= 1,
                        b':' if d2 == 0 => {
                            return Some((&expr[..i], &expr[i + 1..j], &expr[j + 1..]));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Split at a binary operator string, respecting parentheses.
pub(super) fn split_binary_op<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0i32;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let op_len = op_bytes.len();

    if bytes.len() < op_len {
        return None;
    }

    for i in (op_len..=bytes.len()).rev() {
        let start = i - op_len;
        match bytes[start] {
            b')' => depth += 1,
            b'(' => depth -= 1,
            _ => {}
        }
        if depth == 0 && &bytes[start..i] == op_bytes && start > 0 {
            return Some((&expr[..start], &expr[i..]));
        }
    }
    None
}
