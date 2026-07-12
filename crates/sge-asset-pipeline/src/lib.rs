// Copyright The SimpleGameEngine Contributors

//! Source import、disposable cache 与 full Cook 产品管线。

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the private cache becomes reachable through full Cook in the following slice"
    )
)]

mod cache;
mod obj;
