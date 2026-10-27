# Contributing

Bug reports, examples and improvements are welcome. Open an issue to discuss a
substantial change before implementing it.

Use the stable Rust toolchain and run the maintained checks:

```sh
bash scripts/check.sh
```

Website changes also need `bash scripts/build-site.sh`. See
[website build instructions](tools/site/README.md) for its prerequisites.

Every change, including the maintainer's, enters `main` through a pull request
that passes the required hosted `CI` check. The maintainer uses a `contrib/`
branch in this repository. Other contributors use a branch in their fork.
Explain the problem, the resulting behavior and the relevant validation.

Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion in this work, as defined in the Apache-2.0 license, shall
be dual licensed under MIT or Apache-2.0, at the recipient's option, without
any additional terms or conditions.
