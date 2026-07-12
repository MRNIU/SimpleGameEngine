// Copyright The SimpleGameEngine Contributors

//! Source import、disposable cache 与 full Cook 产品管线。

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the canonical parser is consumed by the following import-cache slice"
    )
)]
mod obj;
