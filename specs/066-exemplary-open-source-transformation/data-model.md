# Data Model: Repository Health & Governance Manifest

## Entity: RepositoryHealthManifest
Defines the metadata attributes and compliance standards for TTZip's open-source release.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `project_name` | String | Yes | Name of the project (`TTZip`) |
| `version` | String | Yes | Release semantic version (e.g. `1.0.0`) |
| `license` | String | Yes | Primary license identifier (`MIT`) |
| `supported_platforms` | Array of String | Yes | Supported OS platforms (`macOS 14+ (Sonoma, Sequoia)`) |
| `architectures` | Array of String | Yes | Supported CPU architectures (`arm64`, `x86_64`) |
| `supported_formats_count` | Integer | Yes | Count of full-matrix supported formats (`16`) |
| `ci_runners` | Array of String | Yes | Target CI runner environments (`macos-14`) |
| `has_security_policy` | Boolean | Yes | Flag confirming `SECURITY.md` presence |
| `has_code_of_conduct` | Boolean | Yes | Flag confirming `CODE_OF_CONDUCT.md` presence |
| `has_contributing_guide`| Boolean | Yes | Flag confirming `CONTRIBUTING.md` presence |
