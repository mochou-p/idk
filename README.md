<!-- idk/README.md -->

# idk
*nix process memory editor

## blue 🟦🎯
```sh
# run the target program
cargo run --bin blue

# change the x value so it can be found
[+-*/][number (usize)] # examples: +33, -5, *11, /2

# press [Enter] to print x and exit after red changed it
```

## red 🟥🏹
```sh
# run the memory editor
sudo -E cargo run --bin red [PID]

# use commands to find and rewrite memory of [PID]
```
| command | action |
|-|-|
| `s`/`stack` or `h`/`heap` | selects a memory region for editing |
| `number` | searches the current memory region for values matching `number`<br><br>search is iterative, searching from the list of found addresses so you can find a value that:<br>- keeps changing<br>- you change from the target process<br><br>when only one address matches, a prompt to write to it appears |
| `c`/`clear` | clears the current memory address list |
| `e`/`exit` or empty | exit |

## TODO
- more primitives types than just `usize`
- live updates
- GUI

## License
Licensed under either of
 * Apache License, Version 2.0  
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license  
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions

