# Slices: Public binaries in the Scripts tab

1. **Tracer bullet:** Add a temporary hardcoded release with multiple platform entries to the existing Scripts tab and a working direct download route returning recognizable fixture bytes; prove the UI and route run end to end without Git or SQLite binary storage.
2. **Real publish/download path:** Replace the fixture with authenticated HTTP `PUT` upload and SQLite BLOB persistence, then serve exact uploaded bytes through direct versioned URLs; enforce public anonymous downloads and private-session access.
3. **Versioned multi-asset releases:** Allow multiple filenames per SemVer version, normalize optional `v`, transactionally retain all assets for the three highest versions, reject too-old uploads, and make `/binaries/latest/{filename}` resolve the highest retained matching asset.
4. **Hardening and regression:** Complete filename/header/version validation and error responses within the 512 MiB upload ceiling; verify copy buttons, public/private behavior, repository visibility, and unchanged regular Git smart pushes.
