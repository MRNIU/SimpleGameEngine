// Copyright The SimpleGameEngine Contributors

use sge_reflect::{FieldKey, FieldValues, ReflectedValue, TypeKey, Value};

#[test]
fn reflected_value_wire_rejects_unknown_fields() -> Result<(), Box<dyn std::error::Error>> {
    let value = ReflectedValue::new(TypeKey::new("demo.probe")?, 1, FieldValues::default());
    let mut input = ron::to_string(&value)?;
    assert_eq!(input.pop(), Some(')'));
    input.push_str(",future:true)");

    assert!(ron::from_str::<ReflectedValue>(&input).is_err());
    Ok(())
}

#[test]
fn field_values_wire_rejects_duplicate_keys() -> Result<(), Box<dyn std::error::Error>> {
    let mut fields = FieldValues::default();
    assert_eq!(
        fields.insert(FieldKey::new("speed")?, Value::F32(1.0)),
        None
    );
    let encoded = ron::to_string(&fields)?;
    let duplicated = encoded.replacen('}', ",\"speed\":F32(2.0)}", 1);
    assert_ne!(duplicated, encoded);

    assert!(ron::from_str::<FieldValues>(&duplicated).is_err());
    Ok(())
}
