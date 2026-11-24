# Compiler sans figer le PC

Machine de dev : i7-6700K (4 cœurs / 8 threads), 31 Go de RAM, `/home` en SSD (bfq) presque plein.

## Règle

Toute compilation lourde passe par `phoenix-nice`, directement ou via le shim `cargo`.
Elle peut alors prendre les 8 threads sans provoquer de microfreeze sur les applis.

- `~/.local/bin/phoenix-nice CMD…` lance la commande dans un scope systemd utilisateur :
  - `CPUWeight=idle` (cgroup `cpu.idle=1`) et `chrt -i 0` (SCHED_IDLE) : le noyau ne donne au build que le CPU dont personne d'autre ne veut. Une appli normale qui se réveille passe toujours devant ;
  - `ionice -c3` : les accès disque du build passent après les autres (bfq sur `/home`) ;
  - `MemoryHigh=12G`, `MemoryMax=18G`, `MemorySwapMax=2G` : au-delà de 12 Go, c'est le build qui est ralenti, au lieu de pousser le navigateur ou Discord en swap ;
  - `CARGO_BUILD_JOBS=8` par défaut.
- `~/.local/bin/cargo` est un shim placé avant `~/.cargo/bin` dans le PATH. Il fait passer `build`, `check`, `clippy`, `test`, `bench`, `rustc` et `doc` par `phoenix-nice`. `cargo run` et les autres sous-commandes restent en priorité normale, pour que l'appli lancée reste fluide.
- `phoenix-integrate`, le seul worker de build partagé, utilise `phoenix-nice` avec 8 jobs (il en utilisait 4 avec `nice 19`).

## Mesure (2026-11-24)

On a mesuré le retard de réveil d'un thread normal qui dort 5 ms en boucle (ce que ressent un thread d'interface).

| Situation | p99 | p99.9 | max | réveils > 16 ms |
|---|---|---|---|---|
| Sans build | 1,16 ms | 4,80 ms | 14,0 ms | 0 |
| Pendant `cargo test -p phoenix-app --no-run` (8 jobs, idle) | 0,75 ms | 2,90 ms | 4,99 ms | 0 |

Cette recompilation de `phoenix-app` après un `touch` a pris 30 s.

## Paralléliser les agents

- Les agents travaillent en parallèle, chacun dans son worktree (édition, CDP, docs, captures).
- Les builds, eux, passent par une seule file : `phoenix-submit`, puis `phoenix-integrate`, qui compile le lot une seule fois. Un seul `target/` partagé est possible parce que `/home` n'a que ~3 Go libres.
- Deux builds en même temps sur le même `target/` se bloquent mutuellement (verrou Cargo) et doublent la RAM. Il n'y a donc rien à gagner à en lancer plusieurs : un build à 8 jobs en idle va plus vite.
- Si un agent doit vraiment compiler hors file, il passe par `flock /tmp/phoenix-build.lock` et le shim (`cargo …`), jamais `~/.cargo/bin/cargo` directement.
