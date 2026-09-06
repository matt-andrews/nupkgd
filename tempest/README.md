# Integration tests

The HTTP surface is covered by a [Tempest](https://github.com/matt-andrews/Tempest) suite. Tempest runs in Docker, 
nupkgd runs natively on the host, and the specs talk
to it over plain HTTP exactly as a NuGet client would.

Prerequisites: a Rust toolchain and Docker.

```bash
tempest/run.sh
```

The script builds nupkgd, starts it on port 8080 serving `tempest/fixtures`, wait for `/healthz`,
run the `mattisthegreatest/tempest` image with `tempest/` mounted at `/etc/tests`, stop the server
and exit with Tempest's exit code. Anything after the script name is passed to `tempest test`
### Layout

| Path | Purpose |
| --- | --- |
| `tempest/.env` | `NUPKGD_BASE_URI`, the address Tempest uses to reach the server |
| `tempest/.config.yml` | Shared run options (`base_uri`, retries, reporters) |
| `tempest/*.spec.yml` | One spec per endpoint family plus `client-walk.spec.yml`, which follows links from the service index the way a client does |
| `tempest/fixtures/*.nupkg` | Packages served during the run (committed on purpose; `.gitignore` has an exception) |
| `tempest/fixtures/src/*.nuspec` | Sources for those packages |
| `tempest/fixtures/build-fixtures.ps1` | Rebuilds every `.nupkg` from `src/` |

### Adding a fixture

Drop `<Id>.<Version>.nuspec` into `tempest/fixtures/src/` and run
`.\tempest\fixtures\build-fixtures.ps1`. nupkgd only reads the `.nuspec` inside a package, so the
fixtures contain nothing else. `Broken.Package.1.0.0.nupkg` is deliberately not a zip archive; it
proves unreadable files are ignored at startup.
