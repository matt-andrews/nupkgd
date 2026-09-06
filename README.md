# nupkgd

__nupkgd__ is a local NuGet server for quickly serving packages from one or more folders. This NuGet server is just one 
step away from just using a local folder as your NuGet source, but has the benefit of being able to be used from inside 
a dockerfile.

To start the server: 
```bash
docker run -p 5555:5555 -v "$PWD:/etc/" mattisthegreatest/nupkgd:latest start --recursive
```

you can then add the nuget source with:
```bash
dotnet nuget add source http://localhost:5555/v3/index.json --name nupkgd --allow-insecure-connections
```

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

nupkgd is available under either of the following licenses, at your option:

- [Apache License 2.0](https://github.com/matt-andrews/nupkgd/blob/main/LICENSE-APACHE)
- [MIT License](https://github.com/matt-andrews/nupkgd/blob/main/LICENSE-MIT)
v