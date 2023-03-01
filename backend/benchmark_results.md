The benchmarks where performed with `hyperfine` and the release binary of the 
corresponding commit.

The benchmark data can be created once like this
```bash
cargo build --release
for n in {0..127}; do target/release/discord-gating-bot storage user  add $n 0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA; done
for n in {0..15}; do target/release/discord-gating-bot storage gate add 1073185825484967967 0xcfd3aa1ebc6119d80ed47955a87a9d9c281a97b3 1 0 $n; done
```

And the benchmark itself 
```bash
hyperfine --export-markdown=result.md -n base-urser-reputation-parallel \
'target/release/discord-gating-bot check --config1073185825484967967 42'
```

And the flamegraph
```bash
cargo r --features=profiling
```

| Commit | Command | Mean [s] | Min [s] | Max [s] | 
|:---|:---|---:|---:|---:
|`19829befba117ef35cc01de9150dc4c8758f0217` | `sequential` | 10.316 ± 0.308 | 10.046 | 11.084 | 
|`HEAD` | `base-urser-reputation-parallel` | 5.458 ± 0.111 | 5.298 | 5.639 | 