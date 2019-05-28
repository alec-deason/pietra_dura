To process a tiled .tmx file into an amethyst prefab and save that prefab to the assets directory run:

`cargo run --bin asset_prep --features="asset-prep" raw_assets/map.tmx`


Then to see the map in a bare bones amethyst application run:

`cargo run --bin game`
