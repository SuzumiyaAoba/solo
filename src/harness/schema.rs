//! tool 呼出しの引数を、tool が公開した JSON Schema（`ToolSpec::parameters`）の
//! 解釈できる範囲だけで検査する。oneOf・$ref・format など解釈しない構造は素通し、
//! 確実に分かる違反だけを実行前にモデルへ返す。誤検出で有効な呼出しを止めないことが
//! 網羅性より優先される。
use serde_json::Value;

/// 引数が schema に合えば Ok。違反はモデルが修正して再提案できる文言で返す。
pub(crate) fn check_arguments(parameters: &Value, arguments: &Value) -> Result<(), String> {
    // type/properties/required のどれも無い schema は制約を置かない。
    let expects_object = parameters.get("type").and_then(Value::as_str) == Some("object")
        || parameters.get("properties").is_some()
        || parameters.get("required").is_some();
    if !expects_object {
        return Ok(());
    }
    let Some(arguments) = arguments.as_object() else {
        return Err("引数は JSON オブジェクトで指定してください".into());
    };
    if let Some(required) = parameters.get("required").and_then(Value::as_array) {
        for key in required.iter().filter_map(Value::as_str) {
            // null は省略と同じ扱い（tool 側の任意引数の解釈と揃える）。
            if arguments.get(key).is_none_or(Value::is_null) {
                return Err(format!("必須の引数 `{key}` がありません"));
            }
        }
    }
    let properties = parameters.get("properties").and_then(Value::as_object);
    let closed = parameters.get("additionalProperties") == Some(&Value::Bool(false));
    for (key, value) in arguments {
        match properties.and_then(|properties| properties.get(key)) {
            Some(schema) => check_value(key, value, schema)?,
            None if closed => {
                let mut known: Vec<&str> = properties
                    .map(|properties| properties.keys().map(String::as_str).collect())
                    .unwrap_or_default();
                known.sort_unstable();
                let known = if known.is_empty() {
                    "なし".to_owned()
                } else {
                    known.join(", ")
                };
                return Err(format!(
                    "この tool に `{key}` という引数はありません（有効な引数: {known}）"
                ));
            }
            None => {}
        }
    }
    Ok(())
}

/// 単一の引数値をその schema と照合する。
fn check_value(key: &str, value: &Value, schema: &Value) -> Result<(), String> {
    if let Some(expected) = schema.get("type") {
        let matches = match expected {
            Value::String(kind) => type_matches(kind, value),
            // "type": ["string","null"] のような union 形式。
            Value::Array(kinds) => kinds
                .iter()
                .filter_map(Value::as_str)
                .any(|kind| type_matches(kind, value)),
            _ => true,
        };
        if !matches {
            return Err(format!(
                "`{key}` の型が違います（期待: {}）",
                describe_type(expected)
            ));
        }
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array)
        && !options.is_empty()
        && !options.contains(value)
    {
        let choices = options
            .iter()
            .take(5)
            .map(|option| option.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("`{key}` は {choices} のいずれかで指定してください"));
    }
    Ok(())
}

/// JSON Schema の primitive type 判定。"integer" は f64 の 3.0 を含まない。
fn type_matches(kind: &str, value: &Value) -> bool {
    match kind {
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn describe_type(expected: &Value) -> String {
    match expected {
        Value::String(kind) => kind.clone(),
        Value::Array(kinds) => kinds
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" または "),
        _ => "schema の型".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "offset": {"type": ["integer", "null"], "minimum": 1},
                "mode": {"type": "string", "enum": ["a", "b"]},
            },
            "required": ["path"],
            "additionalProperties": false,
        })
    }

    #[test]
    fn valid_and_invalid_arguments() {
        let schema = schema();
        assert!(check_arguments(&schema, &json!({"path":"a.txt"})).is_ok());
        assert!(check_arguments(&schema, &json!({"path":"a.txt","offset":2})).is_ok());
        assert!(check_arguments(&schema, &json!({"path":"a.txt","mode":"b"})).is_ok());
        // null は任意引数として受け付ける。
        assert!(check_arguments(&schema, &json!({"path":"a.txt","offset":null})).is_ok());

        assert_eq!(
            check_arguments(&schema, &json!({})).unwrap_err(),
            "必須の引数 `path` がありません"
        );
        assert_eq!(
            check_arguments(&schema, &json!({"path":null})).unwrap_err(),
            "必須の引数 `path` がありません"
        );
        assert!(
            check_arguments(&schema, &json!({"path":1}))
                .unwrap_err()
                .contains("string")
        );
        assert!(
            check_arguments(&schema, &json!({"path":"a","offset":1.5}))
                .unwrap_err()
                .contains("`offset` の型")
        );
        assert!(
            check_arguments(&schema, &json!({"path":"a","unknown":1}))
                .unwrap_err()
                .contains("有効な引数: mode, offset, path")
        );
        assert!(
            check_arguments(&schema, &json!({"path":"a","mode":"c"}))
                .unwrap_err()
                .contains("いずれか")
        );
        assert!(
            check_arguments(&schema, &json!("not an object"))
                .unwrap_err()
                .contains("オブジェクト")
        );
    }

    #[test]
    fn uninterpretable_schemas_do_not_reject() {
        // 制約を表さない schema はすべて通す。
        assert!(check_arguments(&json!({}), &json!("anything")).is_ok());
        assert!(check_arguments(&json!({"type":"array"}), &json!([1])).is_ok());
        // additionalProperties が無い・false でない場合は未知の引数も通す。
        let open = json!({"type":"object","properties":{"path":{"type":"string"}}});
        assert!(check_arguments(&open, &json!({"path":"a","extra":1})).is_ok());
        // properties 内の未知構造（minimum 等）は判定しない。
        let nested = json!({
            "type":"object",
            "properties":{"opt":{"type":"integer","minimum":5}},
            "additionalProperties":false,
        });
        assert!(check_arguments(&nested, &json!({"opt":1})).is_ok());
    }
}
