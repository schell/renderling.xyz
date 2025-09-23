# renderling.xyz

The [renderling website](https://renderling.xyz).

## what stuff

This contains a blog, live development devlogs, a manual and documentation.

### manual and docs

The manual and docs are built by hand and checked into the repo.
There's a directory that's gitignore'd for this reason - `renderling`.

`xtask` should take care of everything, but just know that the directory is there and ignored.

## how stuff

  ### developing

  `cargo watch -x 'xtask build'`

  meanwhile 

  `basic-http-server site`

  ### building 

  `cargo xtask --renderling-refresh -e local build`

  ### deploying 

  `cargo xtask --renderling-refresh -e staging deploy`

  `cargo xtask --renderling-refresh -e production deploy`
