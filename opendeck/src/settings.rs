// Settings come from the settings pages (JavaScript) as loose JSON: a number can come as text, an
// empty choice as "" or null. read() turns them into the settings struct of an action and reads each
// field the way JavaScript would: a bool by truthiness, a number like Number(), text like String().
// So "5" is 5 and null is "". serde_json::Value fields get the value as it is.
//
//   #[derive(Deserialize, Default)]
//   #[serde(default, rename_all = "camelCase")]
//   struct Settings { input: String, show_meter: bool, step: f64 }
use serde::Deserialize;
use serde::de::value::{Error, StrDeserializer};
use serde::de::{self, DeserializeOwned, DeserializeSeed, Deserializer, IntoDeserializer, MapAccess, Visitor};
use serde::forward_to_deserialize_any;
use serde_json::{Map, Value};

use crate::js;

/// The settings as a struct. A field that is not in the settings gets its Default.
pub fn read<T: DeserializeOwned + Default>(settings: &Map<String, Value>) -> T {
	T::deserialize(Settings(settings)).unwrap_or_default()
}

/// A message of a settings page as a struct, read like settings.
pub fn read_value<T: DeserializeOwned + Default>(message: &Value) -> T {
	match message {
		Value::Object(map) => read(map),
		_ => T::default(),
	}
}

/// `x || fallback` of JavaScript: 0 and NaN take the fallback.
pub fn or(x: f64, fallback: f64) -> f64 {
	if x == 0.0 || x.is_nan() { fallback } else { x }
}

/// For a number that can be left empty: "" and null are None. Use with
/// `#[serde(deserialize_with = "settings::optional_number")]`.
pub fn optional_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
	let value = Value::deserialize(d)?;
	Ok(match value {
		Value::Null => None,
		Value::String(s) if s.is_empty() => None,
		v => Some(js::number(Some(&v))),
	})
}

struct Settings<'a>(&'a Map<String, Value>);

impl<'de> Deserializer<'de> for Settings<'_> {
	type Error = Error;

	fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		visitor.visit_map(Entries { entries: self.0.iter(), value: None })
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit
		unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any
	}
}

struct Entries<'a> {
	entries: serde_json::map::Iter<'a>,
	value: Option<&'a Value>,
}

impl<'de> MapAccess<'de> for Entries<'_> {
	type Error = Error;

	fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, Error> {
		let Some((key, value)) = self.entries.next() else { return Ok(None) };
		self.value = Some(value);
		let key: StrDeserializer<'_, Error> = key.as_str().into_deserializer();
		seed.deserialize(key).map(Some)
	}

	fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
		seed.deserialize(Loose(self.value.take().unwrap_or(&Value::Null)))
	}
}

/// One value, read the way JavaScript would read it.
struct Loose<'a>(&'a Value);

impl<'de> Deserializer<'de> for Loose<'_> {
	type Error = Error;

	fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		self.0.clone().deserialize_any(visitor).map_err(de::Error::custom)
	}

	fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		visitor.visit_bool(js::truthy(Some(self.0)))
	}

	fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		visitor.visit_f64(js::number(Some(self.0)))
	}

	fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		self.deserialize_f64(visitor)
	}

	fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		visitor.visit_string(js::str_or_empty(Some(self.0)))
	}

	fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		self.deserialize_string(visitor)
	}

	fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		if self.0.is_null() { visitor.visit_none() } else { visitor.visit_some(self) }
	}

	/// A struct inside the settings is read the same way. Anything else than an object is empty.
	fn deserialize_struct<V: Visitor<'de>>(
		self,
		_name: &'static str,
		_fields: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Error> {
		static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);
		let map = self.0.as_object().unwrap_or(&EMPTY);
		visitor.visit_map(Entries { entries: map.iter(), value: None })
	}

	forward_to_deserialize_any! {
		i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 char bytes byte_buf unit unit_struct newtype_struct seq tuple
		tuple_struct map enum identifier ignored_any
	}
}

#[cfg(test)]
pub(crate) mod testing {
	use std::cell::RefCell;

	use super::*;

	/// The keys a settings struct reads.
	pub fn fields<T: DeserializeOwned>() -> Vec<&'static str> {
		struct Fields<'a>(&'a RefCell<Vec<&'static str>>);
		impl<'de> Deserializer<'de> for Fields<'_> {
			type Error = Error;
			fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
				Err(de::Error::custom("not a struct"))
			}
			fn deserialize_struct<V: Visitor<'de>>(
				self,
				_name: &'static str,
				fields: &'static [&'static str],
				_visitor: V,
			) -> Result<V::Value, Error> {
				self.0.borrow_mut().extend(fields);
				Err(de::Error::custom("only the fields"))
			}
			forward_to_deserialize_any! {
				bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit
				unit_struct newtype_struct seq tuple tuple_struct map enum identifier ignored_any
			}
		}
		let found = RefCell::new(Vec::new());
		let _ = T::deserialize(Fields(&found));
		found.into_inner()
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	#[derive(Deserialize, Default, Debug, PartialEq)]
	#[serde(default, rename_all = "camelCase")]
	struct Example {
		input: String,
		show_meter: bool,
		step: f64,
		#[serde(deserialize_with = "optional_number")]
		max: Option<f64>,
		phases: Value,
		inner: Inner,
	}

	#[derive(Deserialize, Default, Debug, PartialEq)]
	#[serde(default)]
	struct Inner {
		port: f64,
	}

	fn example(v: Value) -> Example {
		read_value(&v)
	}

	#[test]
	fn values_are_read_like_javascript_reads_them() {
		let e = example(
			json!({ "input": 12, "showMeter": "0", "step": " 2.5 ", "max": "", "phases": [1], "other": true, "inner": { "port": "4460" } }),
		);
		assert_eq!(
			e,
			Example { input: "12".into(), show_meter: true, step: 2.5, max: None, phases: json!([1]), inner: Inner { port: 4460.0 } }
		);
		assert_eq!(example(json!({ "inner": "x" })).inner, Inner::default());
		let e = example(json!({ "input": null, "showMeter": 0, "step": "x", "max": "7" }));
		assert_eq!((e.input.as_str(), e.show_meter, e.step.is_nan(), e.max), ("", false, true, Some(7.0)));
		assert_eq!(example(json!({})), Example::default());
		assert_eq!(example(json!("not settings")), Example::default());
	}

	#[test]
	fn or_takes_the_fallback_for_zero_and_nan() {
		assert_eq!(or(0.0, 5.0), 5.0);
		assert_eq!(or(f64::NAN, 5.0), 5.0);
		assert_eq!(or(-1.0, 5.0), -1.0);
	}

	#[test]
	fn fields_of_a_settings_struct() {
		assert_eq!(testing::fields::<Example>(), ["input", "showMeter", "step", "max", "phases", "inner"]);
	}
}
