// Copyright The SimpleGameEngine Contributors

pub fn manifest_ron(format_version: u32) -> String {
    format!(
        "(\n    format_version: {format_version},\n    assets: [\n        (\n            id: \"10000000-0000-4000-8000-000000000001\",\n            asset_type: \"sge.mesh\",\n            source: \"Content/a.obj\",\n            importer: Obj(settings: (flip_texcoord_v: false)),\n        ),\n    ],\n)"
    )
}

pub fn manifest_v1_ron_without_importer() -> String {
    "(\n    format_version: 1,\n    assets: [\n        (\n            id: \"10000000-0000-4000-8000-000000000001\",\n            asset_type: \"sge.mesh\",\n            source: \"Content/a.obj\",\n        ),\n    ],\n)"
        .to_owned()
}

pub fn manifest_v2_two_records_ron() -> String {
    manifest_ron(2).replace(
        "        ),\n    ],",
        "        ),\n        (\n            id: \"20000000-0000-4000-8000-000000000002\",\n            asset_type: \"sge.mesh\",\n            source: \"Content/b.obj\",\n            importer: Obj(settings: (flip_texcoord_v: true)),\n        ),\n    ],",
    )
}

pub fn manifest_two_records_ron(format_version: u32) -> String {
    manifest_ron(format_version).replace(
        "        ),\n    ],",
        "        ),\n        (\n            id: \"20000000-0000-4000-8000-000000000002\",\n            asset_type: \"sge.mesh\",\n            source: \"Content/b.obj\",\n            importer: Obj(settings: (flip_texcoord_v: false)),\n        ),\n    ],",
    )
}
