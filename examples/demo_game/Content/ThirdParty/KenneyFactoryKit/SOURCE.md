# Kenney Factory Kit source

- Asset: Factory Kit 3.0, "Completely remade"
- Creator/distributor: Kenney (`www.kenney.nl`)
- Official asset page: <https://kenney.nl/assets/factory-kit>
- Official archive: `kenney_factory-kit_3.0.zip`
- Archive SHA-256: `7e31fb2308e90304672bd15cd18fa9d9f02c03731a8cbc57a8e3e1c181dfb0a7`
- License: Creative Commons Zero (CC0 1.0); see `LICENSE.txt`
- Upstream creation date: 2026-05-01, as recorded in the official license file

Repository-selected files:

- `Models/OBJ format/conveyor-bars-stripe.obj` → `Content/Meshes/Kenney/conveyor-bars-stripe.obj`
  - Upstream SHA-256: `a2456c7e8e7e2c0648c4332ababd709a81246fca0d2bda3adc3393101a310a59`
  - Repository SHA-256 after LF normalization: `ae2b438771804c113157c291e64f3410439fd87981f1e8831696f78b2c91d3cf`
- `Models/OBJ format/Textures/colormap.png` → `Content/Textures/Kenney/colormap.png`
  - SHA-256: `35d7bd6900dde0208429eeaec87fa17fbf024ed59f3f4eab54bc92802eba9dd7`
- `License.txt` → `Content/ThirdParty/KenneyFactoryKit/LICENSE.txt`
  - Repository SHA-256 after LF normalization: `c4fdf0da3738bd130cf1ced541a99660c123f33a962803eff9e7790192765e73`

The OBJ's own `vt` records are the authoritative UVs. SimpleGameEngine does not import its MTL in this slice; the demo scene explicitly binds the mesh and color texture by stable `AssetId`.
