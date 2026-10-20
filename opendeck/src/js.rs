// JavaScript behaviour the plugin needs. settings.rs reads the loose JSON of the settings pages with
// truthy(), number() and to_string(). Numbers turn into text like JavaScript does, so the pictures
// are the same as the ones of the Stream Deck app version.
use serde_json::Value;

/// A missing key is `undefined`, a JSON null is `null`. Some coercions tell them apart.
pub type Opt<'a> = Option<&'a Value>;

pub fn truthy(v: Opt) -> bool {
	match v {
		None | Some(Value::Null) => false,
		Some(Value::Bool(b)) => *b,
		Some(Value::Number(n)) => {
			let f = n.as_f64().unwrap_or(0.0);
			f != 0.0 && !f.is_nan()
		}
		Some(Value::String(s)) => !s.is_empty(),
		Some(Value::Array(_)) | Some(Value::Object(_)) => true,
	}
}

/// Number(value).
pub fn number(v: Opt) -> f64 {
	match v {
		None => f64::NAN,
		Some(Value::Null) => 0.0,
		Some(Value::Bool(b)) => f64::from(u8::from(*b)),
		Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
		Some(Value::String(s)) => string_to_number(s),
		Some(Value::Array(a)) => match a.len() {
			0 => 0.0,
			1 => string_to_number(&to_string(Some(&a[0]))),
			_ => f64::NAN,
		},
		Some(Value::Object(_)) => f64::NAN,
	}
}

/// String.prototype.trim.
pub fn trim(s: &str) -> &str {
	s.trim_matches(is_js_space)
}

pub fn is_js_space(c: char) -> bool {
	matches!(
		c,
		'\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
			..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
	)
}

/// The StringToNumber of JavaScript: "" is 0, " 12 " is 12, "0x1f" is 31, "abc" is NaN.
pub fn string_to_number(s: &str) -> f64 {
	let s = s.trim_matches(is_js_space);
	if s.is_empty() {
		return 0.0;
	}
	for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
		if let Some(digits) = s.strip_prefix(prefix) {
			if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
				return f64::NAN;
			}
			return digits.chars().fold(0.0, |acc, c| acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0)));
		}
	}
	let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
	if unsigned == "Infinity" {
		return if s.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY };
	}
	// Rust also reads "inf" and "nan", JavaScript does not.
	if !unsigned.starts_with(|c: char| c.is_ascii_digit() || c == '.')
		|| !unsigned.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
	{
		return f64::NAN;
	}
	s.parse::<f64>().unwrap_or(f64::NAN)
}

/// String(value). `undefined` is "undefined".
pub fn to_string(v: Opt) -> String {
	match v {
		None => "undefined".into(),
		Some(Value::Null) => "null".into(),
		Some(Value::Bool(b)) => b.to_string(),
		Some(Value::Number(n)) => fmt(n.as_f64().unwrap_or(f64::NAN)),
		Some(Value::String(s)) => s.clone(),
		Some(Value::Array(a)) => {
			a.iter().map(|x| if x.is_null() { String::new() } else { to_string(Some(x)) }).collect::<Vec<_>>().join(",")
		}
		Some(Value::Object(_)) => "[object Object]".into(),
	}
}

/// `String(value ?? "")`.
pub fn str_or_empty(v: Opt) -> String {
	match v {
		None | Some(Value::Null) => String::new(),
		_ => to_string(v),
	}
}

/// Digits and decimal exponent of the shortest form that reads back as the same number.
fn shortest(x: f64) -> (String, i32) {
	let s = format!("{:e}", x.abs());
	let (mantissa, exp) = s.split_once('e').unwrap_or((&s, "0"));
	let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
	(digits, exp.parse().unwrap_or(0))
}

/// Number to text like JavaScript: 1 not "1.0", 1e21 and 1e-7 in exponent form, -0 is "0".
pub fn fmt(x: f64) -> String {
	if x.is_nan() {
		return "NaN".into();
	}
	if x == 0.0 {
		return "0".into();
	}
	if x.is_infinite() {
		return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
	}
	let sign = if x < 0.0 { "-" } else { "" };
	let (digits, exp) = shortest(x);
	let k = digits.len() as i32;
	let n = exp + 1;
	let body = if k <= n && n <= 21 {
		format!("{digits}{}", "0".repeat((n - k) as usize))
	} else if 0 < n && n <= 21 {
		format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
	} else if -6 < n && n <= 0 {
		format!("0.{}{digits}", "0".repeat((-n) as usize))
	} else {
		let e = n - 1;
		let e = if e >= 0 { format!("+{e}") } else { e.to_string() };
		if k == 1 { format!("{digits}e{e}") } else { format!("{}.{}e{e}", &digits[..1], &digits[1..]) }
	};
	format!("{sign}{body}")
}

/// Math.round: a tie goes up (-2.5 becomes -2).
pub fn round(x: f64) -> f64 {
	if !x.is_finite() {
		return x;
	}
	let floor = x.floor();
	let r = if x - floor >= 0.5 { floor + 1.0 } else { floor };
	// Math.round keeps the sign of zero: Math.round(-0.2) is -0.
	if r == 0.0 && x.is_sign_negative() { -0.0 } else { r }
}

/// Seconds since 1970 as an ISO time like `new Date().toISOString()`: 2026-10-19T14:15:16.123Z.
pub fn iso_time(millis: i64) -> String {
	let days = millis.div_euclid(86_400_000);
	let rest = millis.rem_euclid(86_400_000);
	// Civil date from days, after Howard Hinnant's algorithm.
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z - era * 146_097;
	let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
	let y = yoe + era * 400;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let d = doy - (153 * mp + 2) / 5 + 1;
	let m = if mp < 10 { mp + 3 } else { mp - 9 };
	let y = if m <= 2 { y + 1 } else { y };
	format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z", rest / 3_600_000, rest / 60_000 % 60, rest / 1000 % 60, rest % 1000)
}

/// Date.now().
pub fn now() -> i64 {
	std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn numbers_print_like_javascript() {
		for (x, s) in [
			(1.0, "1"),
			(-0.0, "0"),
			(0.1 + 0.2, "0.30000000000000004"),
			(28.8, "28.8"),
			(1e21, "1e+21"),
			(123456789012345680000.0, "123456789012345680000"),
			(1e-7, "1e-7"),
			(0.000001, "0.000001"),
			(-1.5e-9, "-1.5e-9"),
			(f64::INFINITY, "Infinity"),
			(2.5e25, "2.5e+25"),
		] {
			assert_eq!(fmt(x), s, "{x}");
		}
	}

	#[test]
	fn coercions() {
		assert_eq!(number(Some(&json!(" 12 "))), 12.0);
		assert_eq!(number(Some(&json!(""))), 0.0);
		assert!(number(Some(&json!("inf"))).is_nan());
		assert_eq!(number(Some(&json!("-Infinity"))), f64::NEG_INFINITY);
		assert_eq!(number(Some(&json!("0x1f"))), 31.0);
		assert_eq!(number(Some(&json!(null))), 0.0);
		assert!(number(None).is_nan());
		assert_eq!(number(Some(&json!(true))), 1.0);
		assert_eq!(number(Some(&json!("5."))), 5.0);
		assert!(number(Some(&json!("."))).is_nan());
		assert!(!truthy(Some(&json!(0))));
		assert!(truthy(Some(&json!("0"))));
		assert_eq!(round(-2.5), -2.0);
		assert_eq!(round(2.5), 3.0);
		assert_eq!(round(0.49999999999999994), 0.0);
		assert_eq!(iso_time(0), "1970-01-01T00:00:00.000Z");
		assert_eq!(iso_time(1_792_419_316_123), "2026-10-19T14:15:16.123Z");
	}
}
