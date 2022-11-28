![ci](https://github.com/leroyguillaume/projectctl/actions/workflows/ci.yml/badge.svg)

# projectctl

CLI tool to manage project.

## Getting started

- Install from binary
```bash
# Linux x64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/0.1.0/projectctl-0.1.0-linux-x64
# Linux aarch64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/0.1.0/projectctl-0.1.0-linux-aarch64
# MacOS x64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/0.1.0/projectctl-0.1.0-macos-x64

sudo chmod +x /usr/local/bin/projectctl
```

- Install from cargo
```bash
cargo install projectctl
```

- Install from source
```
git clone https://github.com/leroyguillaume/projectctl
cargo install --path projectctl
```

## Documentation

You can run the following command to see all available subcommands:
```bash
projectctl help
```

To display help about a subcommand, you can run:
```bash
projectctl <subcommand> help
```

### Create new project

To create a new project from a template, you can use `new` subcommand.

By default, [leroyguillaume/projectctl-templates](https://github.com/leroyguillaume/projectctl-templates) is used as templates repository. Each directory matches a template. Feel free to open a pull request to add one if you want! You can override it by using `--git` option.

[Liquid](https://shopify.github.io/liquid/) is using as template engine. Each file with `.liquid` extension will be rendered. You can templatize filenames.

projectctl injects some variables:
- `name` that has for value the project name

You can also define any variable you want but keep in mind that you will have to set it when you run command by adding `-d` option (as you can see in following examples).

Example:
```bash
projectctl new \
    -d "description=Awesome new project" \
    -d owner=Me \
    -d "repository-url=https://github.com/my-git-user/my-project-name" \
    rs-simple my-project-name
```
