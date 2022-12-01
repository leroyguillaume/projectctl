![ci](https://github.com/leroyguillaume/projectctl/actions/workflows/ci.yml/badge.svg)

# projectctl

CLI tool to manage project.

## Getting started

- Install from binary

*Sorry for Mac M1/M2 users, I don't have it so I can't build on this architecture because it's not available on GitHub Actions.*

```bash
VERSION=0.1.1

# Linux x64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/v$VERSION/projectctl-$VERSION-linux-x64
# Linux aarch64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/v$VERSION/projectctl-$VERSION-linux-aarch64
# MacOS x64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/v$VERSION/projectctl-$VERSION-macos-x64

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
- `description` that has for value the project description (can be set with `-D` option, unset by default)
- `git_user_name` that has for value the git username (can be undefined if it is not set in default git configuration)
- `git_user_email` that has for value the email of the git user (can be undefined if it is not set in default git configuration)

You can also define any variable you want but keep in mind that you will have to set it when you run command by adding `-d key=value` option.

Example:
```bash
projectctl new rs-lib my-project-name
```
