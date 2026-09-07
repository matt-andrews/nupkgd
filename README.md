<div align="center">
  <h1>nupkgd</h1>

[![Docker Image Size](https://img.shields.io/docker/image-size/mattisthegreatest/nupkgd?style=for-the-badge)](https://hub.docker.com/r/mattisthegreatest/nupkgd)
[![Docker Image Version](https://img.shields.io/docker/v/mattisthegreatest/nupkgd?style=for-the-badge&sort=semver)](https://hub.docker.com/r/mattisthegreatest/nupkgd)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=for-the-badge)](#license)

</div>

__nupkgd__ is a local NuGet server for quickly serving packages from one or more folders. This server is not much different 
from adding a local folder as a NuGet source, but has the added ability to be used in a Dockerfile.

To start the server: 
```bash
docker run -p 5555:5555 -v "$PWD:/etc/" mattisthegreatest/nupkgd:latest start --recursive
```

you can then add the nuget source with:
```bash
dotnet nuget add source http://localhost:5555/v3/index.json --name nupkgd --allow-insecure-connections
```

> [!NOTE]
> `--allow-insecure-connections` is required for an `http` source

or you can use a `nuget.config` file:
```xml
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <add key="nupkgd" value="http://localhost:5555/v3/index.json" protocolVersion="3" allowInsecureConnections="true" />
  </packageSources>
</configuration>
```

## License

nupkgd is available under either of the following licenses:

- [Apache License 2.0](https://github.com/matt-andrews/nupkgd/blob/main/LICENSE-APACHE)
- [MIT License](https://github.com/matt-andrews/nupkgd/blob/main/LICENSE-MIT)
