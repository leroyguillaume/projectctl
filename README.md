![ci](https://github.com/leroyguillaume/projectctl/actions/workflows/ci.yml/badge.svg)

# projectctl

CLI tool to manage project.

## Getting started

- Install from binary

*Sorry for Mac M1/M2 users, I don't have it so I can't build on this architecture because it's not available on GitHub Actions.*

```bash
VERSION=1.0.0

# Linux x64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/v$VERSION/projectctl-$VERSION-linux-x64
# Linux aarch64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/v$VERSION/projectctl-$VERSION-linux-aarch64
# MacOS x64
curl -Lfo /usr/local/bin/projectctl https://github.com/leroyguillaume/projectctl/releases/download/v$VERSION/projectctl-$VERSION-macos-x64

sudo chmod +x /usr/local/bin/projectctl

# Allow projectctl to source environment variables automatically when you're entering into a directory present in ~/.projectctl/allowed-dirs
# If you're using bash
echo 'eval "$(projectctl hook bash)"' >> ~/.bashrc
source ~/.bashrc
# If you're using zsh
echo 'eval "$(projectctl hook zsh)"' >> ~/.zshrc
source ~/.zshrc
```

- Install from cargo
```bash
cargo install projectctl

# Allow projectctl to source environment variables automatically when you're entering into a directory present in ~/.projectctl/allowed-dirs
# If you're using bash
echo 'eval "$(projectctl hook bash)"' >> ~/.bashrc
source ~/.bashrc
# If you're using zsh
echo 'eval "$(projectctl hook zsh)"' >> ~/.zshrc
source ~/.zshrc
```

- Install from source
```bash
git clone https://github.com/leroyguillaume/projectctl
cargo install --path projectctl

# Allow projectctl to source environment variables automatically when you're entering into a directory present in ~/.projectctl/allowed-dirs
# If you're using bash
echo 'eval "$(projectctl hook bash)"' >> ~/.bashrc
source ~/.bashrc
# If you're using zsh
echo 'eval "$(projectctl hook zsh)"' >> ~/.zshrc
source ~/.zshrc
```

### Project

To create a new project from a template, you can use `new` subcommand.

By default, [leroyguillaume/projectctl-templates](https://github.com/leroyguillaume/projectctl-templates) is used as templates repository. Each directory matches a template. Feel free to open a pull request to add one if you want! You can override it by using `--git` option.

[Liquid](https://shopify.github.io/liquid/) is using as template engine. Each file with `.liquid` extension will be rendered. You can templatize filenames.

projectctl injects some variables:
- `name` that has for value the project name
- `description` that has for value the project description (can be set with `-d` option, unset by default)
- `git.user.name` that has for value the git username (can be undefined if it is not set in default git configuration)
- `git.user.email` that has for value the email of the git user (can be undefined if it is not set in default git configuration)

You can also define any variable you want but keep in mind that you will have to set it when you run command by adding `--values` option.

Examples:
```bash
projectctl new rs-lib my-project-name
projectctl new --values '{"repository-url":"https://github.com/username/project-name"}' rs-lib my-project-name
```

projectctl automatically updated `~/.projectctl/allowed-dirs`.

When you want to delete a project, you can run the following command to make sure everything is clean-up:
```bash
projectctl destroy my-project-name
```

### Configuration files

When you run `projectctl env`, these configuration files are loaded (from least to most priority):
- `projectctl.yml`
- `projectctl.local.yml`

Note that you can override these locations with option `-c` (or `--config`).

All configuration files must match [this JSON schema](resources/main/config.schema.json).

You can find [here](examples/) some configuration examples.
