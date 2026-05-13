# Plume
https://github.com/atomicdata-dev/atomic-server

## Running dfpp
Follow steps from [here](https://www.notion.so/justus-adam/Atomic-Data-applying-DFPP-b1f3d4d6a45b4387a8c0c824c367ea3c).

In `plume-models`:
Pre-bug fix
`cargo dfpp --external-annotations external-annotations.toml  --verbose -- --lib --no-default-features --features postgres` 

Post-bug fix
`cargo dfpp --external-annotations external-annotations.toml  --verbose -- --lib --no-default-features --features postgres --features delete-comments` 

Note: running this may sometimes return `could not resolve...` for the external annotations. Currently do not know why :( 
