# dotpick

[![CI](https://github.com/rvben/dotpick/actions/workflows/ci.yml/badge.svg)](https://github.com/rvben/dotpick/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/dotpick.svg)](https://crates.io/crates/dotpick)
[![clispec compliant](https://img.shields.io/badge/clispec-compliant-3b82f6)](https://clispec.dev)

Token-minimal field projection over JSON, YAML, TOML and NDJSON. Select fields
by dotpath and emit the smallest valid slice. The anti-jq for agents and
scripts: pure projection and format conversion, with no expression language to
get wrong.

## Why

`jq` and `yq` dump whole payloads, speak cryptic error messages, and `jq`
cannot even read YAML or TOML. When you only need three fields out of a 50 KB
response, you pay for the other 49 KB. `dotpick` takes a simple list of
dotpaths and returns just those fields, in the format you ask for.

```sh
# 50 KB of pod JSON in, one name per line out
kubectl get pods -o json | dotpick '.items[].metadata.name' -o raw
```

## Install

```sh
cargo install dotpick
```

## Quickstart

```sh
# Smallest sub-document of two fields (structure preserved)
echo '{"metadata":{"name":"web","ns":"prod"},"spec":{"replicas":3}}' \
  | dotpick '.metadata.name,.spec.replicas'
# => {"metadata":{"name":"web"},"spec":{"replicas":3}}

# Just the value, unquoted
dotpick .spec.replicas deploy.yaml -o raw
# => 3

# Flatten to leaf names
dotpick '.metadata.name,.spec.replicas' deploy.yaml --flat
# => {"name":"web","replicas":3}

# Stream array elements as NDJSON
cat pods.json | dotpick '.items[]' -o ndjson
# => {"name":"a"}
#    {"name":"b"}

# Convert formats with the root path
dotpick . config.toml -o yaml
```

`dotpick <paths> [file]` is shorthand for `dotpick project <paths> [file]`.

## Dotpath grammar

| Syntax            | Meaning                                             |
| ----------------- | --------------------------------------------------- |
| `.key`            | object key (bareword `[A-Za-z0-9_-]+`)              |
| `["any.key"]`     | quoted key, for keys with dots, spaces or brackets  |
| `[0]`             | array index (non-negative)                          |
| `[]`              | iterate every element of an array                   |
| `.`               | the whole document (useful for format conversion)   |
| `.a.b[0].c[].d`   | chain segments freely                               |
| `.a.b,.c[].d`     | comma-separate multiple paths                       |

## Output shapes

- **structured** (default): the smallest sub-document that keeps the original
  nesting.
- **`--flat`**: an object keyed by each path's final name.
- **`-o raw`**: bare scalar values, one per line (great for shell capture).
- **`-o ndjson`**: one compact JSON value per selected match; `[]` controls
  granularity (`.items[]` streams elements, `.items[].name` streams names).

Object keys are emitted in sorted order for stable, diff-friendly output.

## Formats

Input format is auto-detected (or taken from the file extension, or forced with
`--from json|yaml|toml|ndjson`). Output is selected with `-o`/`--output`
(`auto`, `json`, `yaml`, `toml`, `ndjson`, `raw`); `auto` is JSON, or NDJSON
when the input is NDJSON. `--to` is an alias for `--output`. Use `--pretty` for
indented JSON.

## Missing fields

By default a missing path is an error with a "nearest existing key" hint:

```sh
echo '{"spec":{"replic":3}}' | dotpick .spec.replicas
# stderr: {"error":{"kind":"path_not_found","message":"path .spec.replicas not found; nearest existing: replic", ...}}
```

Pass `--allow-missing` to skip absent paths instead.

## Exit codes

| Code | Meaning                                                        |
| ---- | ------------------------------------------------------------- |
| `0`  | success                                                       |
| `1`  | no match (a selected path is absent, or a wrong-typed segment)|
| `2`  | parse, serialize, or IO failure                              |
| `3`  | usage error (bad arguments, bad dotpath, name collision, raw on non-scalar) |

## For agents (clispec)

dotpick follows [The CLI Spec](https://clispec.dev): data on stdout, structured
error envelopes on the last line of stderr, and a `schema` subcommand whose
output validates against `clispec.dev/schema/v0.2.json` (checked by the test
suite). Every command is marked read-only (`mutating: false`).

```sh
dotpick schema   # machine-readable contract: commands, args, error kinds, exit codes
```

dotpick is a stateless filter, so the list-oriented principles (pagination,
resource conflicts) do not apply; its `paths` argument is itself the field
selector.

## License

MIT
