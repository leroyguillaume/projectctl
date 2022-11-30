# projectctl contributing guidelines

Thank you for your interest in making projectctl better! We'd love to have your contribution.

## License

projectctl is licenced under the MIT license. Please see the [LICENSE](LICENSE) file for more details.

## Pull Requests

To make changes to projectctl, please send in pull requests on GitHub to the `develop`
branch. We'll review them and either merge or request changes. GitHub Actions tests
everything as well, so you may get feedback from it too.

If you make additions or other changes to a pull request, feel free to either amend
previous commits or only add new ones, however you prefer. We may ask you to squash
your commits before merging, depending.

## Development workflow

### Software requirements
- [Rust](http://rust-lang.org/install.html)
- [pre-commit](https://pre-commit.com/#install)
- [npm](https://nodejs.org/en/download/)

```bash
git clone https://github.com/leroyguillaume/projectctl
cd projectctl
pre-commit install -t pre-commit -t commit-msg
npm install
cargo build
```

To run the tests:
```bash
cargo test
```
