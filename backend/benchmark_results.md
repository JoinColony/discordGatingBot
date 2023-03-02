The benchmarks where performed with `hyperfine` and the release binary of the 
corresponding commit.

First build the release binary

```bash
cargo build --release
```


The benchmark data can be created once like this
```bash
rm -ri bench_data
for n in {0..127}; do 
    target/release/discord-gating-bot \
        --config-file ./bench-config.toml storage user  add \
        $n 0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA; 
done

for n in {0..15}; do 
    target/release/discord-gating-bot \
        --config-file ./bench-config.toml storage gate add \
        0 0xcfd3aa1ebc6119d80ed47955a87a9d9c281a97b3 1 0 $n;
done
```

And the benchmark itself 
```bash
hyperfine --export-markdown=result.md -n base-urser-reputation-parallel \
'target/release/discord-gating-bot --config-file ./bench-config.toml check 0 42'
```

And the flamegraph
```bash
cargo r --features=profiling -- --config-file ./bench-config.toml check 0 42
```
## Results for 16 gates

| Commit | Command | Mean [s] | Min [s] | Max [s] | 
|:---|:---|---:|---:|---:|
|`19829befba117ef35cc01de9150dc4c8758f0217` | `sequential` | 10.316 ± 0.308 | 10.046 | 11.084 | 
|`3a6b62534279d6c295922f277293041228b73f91` | `base-urser-reputation-parallel` | 5.458 ± 0.111 | 5.298 | 5.639 | 
|`HEAD`| `parallel-gates` | 1.330 ± 0.048 | 1.264 | 1.418 | 
