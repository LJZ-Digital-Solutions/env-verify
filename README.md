# env-verify
**Validate your environment or fail fast.**

`env-verify` is a standalone tool that can be used to verify that your run- or buildtime environment is set up correctly *before* causing any problems. It allows you to model your environment using a standardized [JSON schema](https://json-schema.org/) and supports basic substitution so that you can leave your dirty bash scripts at home.

## Why?

Many CI workflows and programs use an arbitrary number of funnily named environment variables  (e.g. `RELEASE_VERSION`, `GITHUB_TOKEN`, `SYNC_PERIOD`, etc.) to configure their behavior. This is:

- **slow**, as the environment is often checked late in a workflow's lifetime
- **cumbersome**, as the valid values for an environment variable need to be documented in READMEs and docs
- **wet**, as environment parsing logic needs to be reimplemented in each workflow separately
- **insecure**, as wrong or missing environment variables can cause undefined behavior

*Add some github actions quirks in the mix, and you get a mess...*

With `env-verify`, you can define your environment as [JSON schema](https://json-schema.org/) and store your environment specification with your code while keeping variables safe and secret with your git provider. It will:

- Validate your input environment against a specified JSON schema
- Substitute environment variables and secrets if needed
- Output the validated and substituted environment to a file if desired

This is best shown with some examples, see below.

## Usage

`env-verify` is available as reusable Github action step and pre-built binary. At minimum, it requires two files:

- A JSON schema file, describing your environment (`env.schema.json`)
    - This should be stored in git, together with your code
- A JSON environment file, holding your actual environment variables (`env.json`)
    - This should be dynamically generated from your environment, or stored in git with **substitutions** (see examples below)

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

## Examples

You can find more examples in the [./tests](./tests) directory.

### Basic Environment Validation

Validate three mandatory fields with different types.

**env.schema.json**
```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "properties": {
    "app_name": {
      "type": "string",
      "minLength": 1
    },
    "debug": {
      "type": "boolean"
    },
    "port": {
      "type": "integer",
      "minimum": 1,
      "maximum": 65535
    }
  },
  "required": ["app_name", "debug", "port"]
}
```

Valid **env.json**
```json
{
  "app_name": "my-service",
  "debug": true,
  "port": 8080
}
```

**CLI usage**:
```bash
env-verify --schema env.schema.json --input env.json
```

**Github Actions usage**:
```yaml
- name: Validate basic environment
  uses: LJZ-Digital-Solutions/env-verify@main
  with:
    schema: 'env.schema.json'
    input: 'env.json'
```

### Environment Variable Substitution

Environment values get substituted into the actual environment before validation.

**env.schema.json**
```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "properties": {
    "environment": {
      "type": "string",
      "enum": ["development", "staging", "production"]
    },
    "version": {
      "type": "string",
      "pattern": "^v[0-9]+\\.[0-9]+\\.[0-9]+$"
    },
    "branch": {
      "type": "string",
      "minLength": 1
    }
  },
  "required": ["environment", "version", "branch"]
}
```

Valid **env.json**
```json
{
  "environment": "{{ deploy_env }}",
  "version": "{{ release_tag }}",
  "branch": "{{ github_ref }}"
}
```

**vars.json** (This file should be generated from your environment)
```json
{
  "deploy_env": "production",
  "release_tag": "v1.2.3",
  "github_ref": "main"
}
```

**CLI usage**:
```bash
env-verify --schema env.schema.json --input env.json --env-vars vars.json
```

**Github Actions usage**:
```yaml
- name: Validate substituted environment
  uses: LJZ-Digital-Solutions/env-verify@main
  with:
    schema: 'env.schema.json'
    input: 'env.json'
    env-vars: ${{ toJSON(vars) }}
```

### Environment Variable and Secret Substitution

Environment variables and environment secrets get substituted before validation. The validated environment is written to "validated-db-config.json"

**env.schema.json**
```json
{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "type": "object",
  "properties": {
    "host": {
      "type": "string",
      "format": "hostname"
    },
    "port": {
      "type": "integer",
      "minimum": 1,
      "maximum": 65535
    },
    "username": {
      "type": "string",
      "minLength": 1
    },
    "password": {
      "type": "string",
      "minLength": 8
    },
    "ssl_enabled": {
      "type": "boolean"
    }
  },
  "required": ["host", "port", "username", "password"]
}
```

Valid **env.json**
```json
{
  "host": "{{ db_host }}",
  "port": 5432,
  "username": "{{ db_user }}",
  "password": "{{ db_password }}",
  "ssl_enabled": true
}
```

**vars.json** (This file should be generated from your environment)
```json
{
  "db_host": "localhost",
  "db_user": "dev_user",
  "db_password": "weak_password"
}
```

**secrets.json** (This file should be generated from your environment)
```json
{
  "db_host": "prod.database.company.com",
  "db_password": "super_secure_prod_password_123!"
}
```

**CLI usage**:
```bash
env-verify --schema env.schema.json --input env.json \
  --env-vars vars.json --env-secrets secrets.json \
  --output validated-db-config.json
```

**Github Actions usage**:
```yaml
- name: Validate substituted environment
  uses: LJZ-Digital-Solutions/env-verify@main
  with:
    schema: 'env.schema.json'
    input: 'env.json'
    env-vars: ${{ toJSON(vars) }}
    env-secrets: ${{ toJSON(secrets) }}
    output: 'validated-db-config.json'
```

