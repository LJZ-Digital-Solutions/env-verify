# env-verify
**Validate your environment or fail fast.**

`env-verify` is a standalone tool that can be used to verify that your run- or buildtime environment is set up correctly *before* causing any problems. It allows you to model your environment using a standardized [JSON schema](https://json-schema.org/) and supports basic substitution so that you can leave your dirty bash scripts at home.

## Usage

`env-verify` is available as reusable Github action step and pre-built binary.

### Github Actions Step

You can define your environment as a JSON schema and push this schema to VCS. Then, right after checking out your code, you can use the defined schema for validation like so:

```yaml
- name: Validate configuration
  uses: LJZ-Digital-Solutions/env-verify@main
  with:
    schema: 'env.schema.json'                          # path to JSON schema (see https://json-schema.org/)
    input: 'env.json'                                  # path to JSON environment (can be part of VCS, or generated from an environment variable)
    env-vars: ${{ toJSON(vars) }}                      # allow environment variable substitution )optional) 
    env-secrets: ${{ toJSON(secrets) }}                # allow environment secret substitution (optional)
    output: 'validated-config.json'                    # path to write substituted and verified environment to (optional)
    version: 'latest'                                  # defaults to 'latest' (optional)
```

### Prebuilt Binaries

You can use prebuilt binaries (or compile from source) and use the command line interface to verify your environment as follows:

```bash
env-verify \
  --schema env.schema.json \          # path to JSON schema (see https://json-schema.org/)
  --input env.json \                  # path to JSON environment (can be part of VCS, or generated from an environment variable)
  --env-vars vars.json \              # allow environment variable substitution )optional) 
  --env-secrets secrets.json \        # allow environment secret substitution (optional)
  --output validated-config.json      # path to write substituted and verified environment to (optional)
```

You can find examples in the [./tests](./tests) directory.
