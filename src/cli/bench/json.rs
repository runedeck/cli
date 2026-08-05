use serde::{Serialize, Serializer};

/// A number that serializes the way `JSON.stringify` prints `JS` numbers:
/// integer-valued floats below 1e21 emit as plain integers (`0`, `42`,
/// `10000000000000000`), matching ECMAScript's decimal notation for that
/// range. Non-integral values use `serde_json`'s shortest round-trip form,
/// which matches ECMAScript except in its exponential zones (below 1e-6 and
/// at 1e21 and above) — a divergence the compat spec records; no metric this
/// runner emits reaches those zones while provider costs stay at zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JsNumber(pub f64);

const ES_EXPONENTIAL_THRESHOLD: f64 = 1e21;

impl Serialize for JsNumber {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = self.0;
        if value.is_finite() && value.fract() == 0.0 && value.abs() < ES_EXPONENTIAL_THRESHOLD {
            #[allow(clippy::cast_possible_truncation)]
            serializer.serialize_i128(value as i128)
        } else {
            serializer.serialize_f64(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JsNumber;

    fn rendered(value: f64) -> String {
        serde_json::to_string(&JsNumber(value)).expect("number renders")
    }

    #[test]
    fn integral_values_render_like_js() {
        assert_eq!(rendered(0.0), "0");
        assert_eq!(rendered(-0.0), "0");
        assert_eq!(rendered(100.0), "100");
        assert_eq!(rendered(1e16), "10000000000000000");
        assert_eq!(rendered(-42.0), "-42");
    }

    #[test]
    fn non_integral_values_render_shortest() {
        assert_eq!(rendered(0.5), "0.5");
        assert_eq!(rendered(34.5), "34.5");
    }
}
