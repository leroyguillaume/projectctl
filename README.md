# projectctl

CLI to make easier projects automation and environment management.

## Description

`projectctl` automate some DevOps actions for you.

All project information is stored in `.projectctl/project.json`.

## Getting started

```bash
git clone https://github.com/leroyguillaume/projectctl.git
cargo install --path projectctl
```

## Usage

### Render template

```bash
projectctl render --help
```

Render [Liquid](https://shopify.github.io/liquid/) template and add the destination file to `rendered` section in project file.

You can find template examples [here](examples/templates).

#### Filters

All [Liquid stdlib filters](https://shopify.github.io/liquid/filters) are available in addition to:
- `json_encode`: encodes a variable to JSON
- `json_encode_pretty`: encode a variable to pretty JSON

#### Variables

##### `env`

Environment variables.

Example: `env.HOME`

##### `git`

Configuration of the git repository. If it is not, only the global configuration is loaded.

The dot (`.`) in configuration key is replaced by `_`.

Example: `git.user_name`

##### `project`

Project metadata. You can find all available fields in `metadata` section in project file (`.projectctl/project.json`).

Example: `project.name`

##### `var`

Custom variables from CLI.

Example: `var.categories`

#### Example

```bash
# Render from a git repository
projectctl render --git https://github.com/leroyguillaume/projectctl.git -t examples/templates/Cargo.toml.liquid Cargo.toml

# Render from a URL
projectctl render --url https://raw.githubusercontent.com/leroyguillaume/projectctl/main/examples/templates/Cargo.toml.liquid Cargo.toml

# Render from a local file (must be located in project directory)
projectctl render --local -t Cargo.toml.liquid Cargo.toml
```

### Update

```bash
projectctl update --help
```

Update previously rendered files.

If the file changed since last rendering, it will be not overwriten.

#### Example

```bash
projectctl update
```

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).
