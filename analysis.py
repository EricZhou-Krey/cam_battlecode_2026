#!/usr/bin/env python3
"""
CAMBC champion analysis
=======================

Produces three figures:

  1. damage_per_turn.png  - per-turn damage of every champion (small multiples).
                            Stacked: raw damage / Ren-style reflect / stun credit.
  2. damage_summary.png   - average damage per turn, raw vs. stun-adjusted.
  3. fight_heatmap.png    - 1v1 duel heatmap. Colour = winner's remaining HP
                            (red = barely won, green = won with full HP).

Run:   python analysis.py            (saves PNGs next to the script)
       python analysis.py --show     (also opens the windows)
       python analysis.py --turns 32 --out results/

Modelling assumptions (all in one place so they are easy to change)
-------------------------------------------------------------------
* Base stats only (no shards / emblems / items). Stats come from the handbook.
* Each turn a body makes one basic attack and casts at most one ability.
  Ability priority: stun > damaging stun > damage > shield > reset.
* Cooldown is 8 owner turn ends (cast on turn 1, castable again on turn 9).
* Attack resets give a second basic attack in the same turn (the attack is
  made first, then the reset, then the second attack). Marcella's second
  attack (the "enhanced" one) stuns.
* A stun costs the victim its next turn (-1 turn). Marcella is immune.
* Ranged units get a +2 turn headstart on a *pure melee* champion that has no
  dash (the melee unit loses its first 2 turns closing the gap). If the melee
  champion has a dash, there is no headstart.
* Everyone stands still and trades: no walking, no kiting, no recall.
* Cassi & Issac are one fighter with two bodies. They share one HP pool equal
  to the LOWER of the two (Cassi: 700), and both bodies attack every turn.
  A stun hits one body only (the more threatening, un-stunned one).
* Ren retaliates for 0.2 x Focus on every instance of damage he takes (even
  the one that kills him). A mutual kill is a draw.
* Varg's shield (2.5 x Focus) soaks damage and lasts 5 of his turn ends.
* Willow's heal needs an ally so it is a no-op in 1v1; Ryth's passive adds 1%
  of bonus health to Strength (0 here because there are no shards).
* Prism is evaluated with 0 / 10 / 30 / 60 / 100 carried gems
  (+0.2 Focus per gem).
* "Damage per turn" graphs fight a training dummy that never attacks back.
  Stun credit = stuns landed x the average damage-per-turn of the roster
  (the damage the victim would have dealt). Reflect credit = 0.2 x Focus x the
  roster's average number of damage instances per turn.
"""

import argparse
import os
import sys
from dataclasses import dataclass, field

import numpy as np
import matplotlib

if "--show" not in sys.argv:
    matplotlib.use("Agg")
import matplotlib.pyplot as plt

# --------------------------------------------------------------------------
# Data (straight from the handbook)
# --------------------------------------------------------------------------
COOLDOWN = 8
HEADSTART_TURNS = 2
SHIELD_TURNS = 5
PRISM_GEMS = [0, 10, 30, 60, 100]

# ability kinds:
#   dmg      : mult x Focus damage
#   dmgstun  : mult x Focus damage + 1-round stun
#   stun     : 1-round stun, no damage
#   reset    : attack reset
#   enh_reset: attack reset, next attack stuns
#   shield   : mult x Focus shield
#   dash     : movement (no value in a stationary duel, but counts as a dash)
#   heal     : ally heal (no-op in 1v1)
CHAMPS = {
    "Ryth":         dict(hp=1200, st=50, fo=100, ranged=False,
                         ab=[("dmgstun", 1.0), ("reset", 0)]),
    "Vesper Vale":  dict(hp=900,  st=45, fo=100, ranged=True,
                         ab=[("reset", 0), ("dash", 0)]),
    "Prism":        dict(hp=700,  st=35, fo=80,  ranged=True,
                         ab=[("dmgstun", 1.5), ("dmg", 1.0)]),
    "Stellar":      dict(hp=750,  st=35, fo=100, ranged=True,
                         ab=[("dmg", 2.0), ("dmg", 1.4)]),
    "Vendetta":     dict(hp=800,  st=70, fo=100, ranged=False,
                         ab=[("reset", 0), ("dash", 0)]),
    "Marcella":     dict(hp=1200, st=45, fo=50,  ranged=False, immune=True,
                         ab=[("enh_reset", 0), ("dmg", 2.0)]),
    "Tavi":         dict(hp=750,  st=35, fo=100, ranged=True,
                         ab=[("reset", 0), ("dmgstun", 2.2)]),
    "Willow":       dict(hp=700,  st=45, fo=105, ranged=True,
                         ab=[("heal", 0), ("reset", 0)]),
    "Ren":          dict(hp=900,  st=40, fo=100, ranged=False, retaliate=0.2,
                         ab=[("dmgstun", 1.25), ("dash", 0)]),
    "Varg":         dict(hp=1000, st=50, fo=100, ranged=False,
                         ab=[("reset", 0), ("shield", 2.5)]),
}
CASSI = dict(hp=700, st=30, fo=70, ranged=True,
             ab=[("dmg", 2.0), ("stun", 0)])
ISSAC = dict(hp=900, st=40, fo=100, ranged=False,
             ab=[("reset", 0), ("dash", 0)])

PAIR = "Cassi & Issac"
ALL_NAMES = ([PAIR, "Ryth", "Vesper Vale"]
             + [f"Prism ({g} gems)" for g in PRISM_GEMS]
             + ["Stellar", "Vendetta", "Marcella", "Tavi", "Willow", "Ren", "Varg"])
BASE_ROSTER = [PAIR, "Ryth", "Vesper Vale", "Prism (0 gems)", "Stellar",
               "Vendetta", "Marcella", "Tavi", "Willow", "Ren", "Varg"]

PRIORITY = ["stun", "dmgstun", "dmg", "shield", "enh_reset", "reset"]
DAMAGE_KINDS = ("dmg", "dmgstun")


# --------------------------------------------------------------------------
# Simulation engine
# --------------------------------------------------------------------------
@dataclass
class Body:
    name: str
    strength: float
    focus: float
    ranged: bool
    abilities: list
    immune: bool = False
    retaliate: float = 0.0
    cds: list = field(default_factory=list)
    stun: int = 0
    delay: int = 0

    def __post_init__(self):
        self.cds = [0] * len(self.abilities)

    @property
    def has_dash(self):
        return any(k == "dash" for k, _ in self.abilities)

    @property
    def threat(self):
        best = max([m * self.focus for k, m in self.abilities
                    if k in DAMAGE_KINDS], default=0.0)
        return self.strength + best


class Fighter:
    """One or more bodies sharing a single HP pool."""

    def __init__(self, label, bodies, hp):
        self.label = label
        self.bodies = bodies
        self.hp = float(hp)
        self.maxhp = float(hp)
        self.shield = 0.0
        self.shield_t = 0

    @property
    def pure_melee_no_dash(self):
        return all((not b.ranged) and (not b.has_dash) for b in self.bodies)

    @property
    def any_ranged(self):
        return any(b.ranged for b in self.bodies)


def _body_from_spec(name, s):
    strength = s["st"]  # Ryth passive: +1% bonus health; no shards -> +0
    return Body(name, strength, s["fo"], s["ranged"], list(s["ab"]),
                immune=s.get("immune", False), retaliate=s.get("retaliate", 0.0))


def make(name):
    if name == PAIR:
        bodies = [_body_from_spec("Cassi", CASSI), _body_from_spec("Issac", ISSAC)]
        # both die together -> the lower HP is the shared pool
        return Fighter(name, bodies, min(CASSI["hp"], ISSAC["hp"]))
    if name.startswith("Prism"):
        gems = int(name.split("(")[1].split()[0])
        spec = dict(CHAMPS["Prism"])
        spec["fo"] = spec["fo"] + 0.2 * gems
        return Fighter(name, [_body_from_spec("Prism", spec)], spec["hp"])
    spec = CHAMPS[name]
    return Fighter(name, [_body_from_spec(name, spec)], spec["hp"])


def make_dummy():
    b = Body("Dummy", 0, 0, False, [])
    return Fighter("Dummy", [b], 1e12)


def hit(src, dst, amount):
    """Apply damage; Ren-style retaliation fires on any damage taken."""
    if amount <= 0 or dst.hp <= 0 or src.hp <= 0:
        return
    if dst.shield > 0:
        absorbed = min(dst.shield, amount)
        dst.shield -= absorbed
        amount -= absorbed
    if amount <= 0:
        return
    dst.hp -= amount
    refl = sum(b.retaliate * b.focus for b in dst.bodies)
    if refl:
        src.hp -= refl  # still fires if dst just died


def body_actions(me, body, opp):
    """One body's turn. Returns (damage_dealt, damage_instances, stuns)."""
    dmg = hits = stuns = 0.0
    tgt = max(opp.bodies, key=lambda b: (b.stun == 0, b.threat))

    def apply_stun():
        nonlocal stuns
        if not tgt.immune and tgt.stun == 0:
            tgt.stun = 1
            stuns += 1

    def strike(stun=False):
        nonlocal dmg, hits
        if opp.hp <= 0 or me.hp <= 0:
            return
        if body.strength > 0:
            hit(me, opp, body.strength)
            dmg += body.strength
            hits += 1
        if stun:
            apply_stun()

    strike()  # basic attack

    # choose ONE ability this turn
    choice = None
    for kind in PRIORITY:
        for i, (k, m) in enumerate(body.abilities):
            if k != kind or body.cds[i] > 0:
                continue
            if k == "stun" and (tgt.immune or tgt.stun > 0):
                continue
            if k == "shield" and me.shield > 0:
                continue
            choice = (i, k, m)
            break
        if choice:
            break

    if choice and me.hp > 0 and opp.hp > 0:
        i, k, m = choice
        body.cds[i] = COOLDOWN
        if k == "stun":
            apply_stun()
        elif k in DAMAGE_KINDS:
            amt = m * body.focus
            hit(me, opp, amt)
            dmg += amt
            hits += 1
            if k == "dmgstun":
                apply_stun()
        elif k == "shield":
            me.shield = m * body.focus
            me.shield_t = SHIELD_TURNS
        elif k in ("reset", "enh_reset"):
            strike(stun=(k == "enh_reset"))
    return dmg, hits, stuns


def take_turn(me, opp):
    dmg = hits = stuns = 0.0
    for body in me.bodies:
        if me.hp <= 0 or opp.hp <= 0:
            break
        if body.delay > 0:
            body.delay -= 1
        elif body.stun > 0:
            body.stun -= 1
        else:
            d, h, s = body_actions(me, body, opp)
            dmg += d
            hits += h
            stuns += s
        # timers tick at turn end (including the turn the ability was cast)
        body.cds = [max(0, c - 1) for c in body.cds]
        if me.shield_t > 0:
            me.shield_t -= 1
            if me.shield_t == 0:
                me.shield = 0.0
    return dmg, hits, stuns


def apply_headstart(a, b):
    for x, y in ((a, b), (b, a)):
        if x.any_ranged and y.pure_melee_no_dash:
            for bd in y.bodies:
                bd.delay = HEADSTART_TURNS


def duel(name_a, name_b, max_rounds=600):
    """Returns (winner: 0/1/None for draw, winner remaining-HP fraction, rounds)."""
    a, b = make(name_a), make(name_b)
    apply_headstart(a, b)
    rounds = 0
    for r in range(max_rounds):
        rounds = r + 1
        first, second = (a, b) if r % 2 == 0 else (b, a)  # starting team alternates
        take_turn(first, second)
        if a.hp <= 0 or b.hp <= 0:
            break
        take_turn(second, first)
        if a.hp <= 0 or b.hp <= 0:
            break
    if a.hp <= 0 and b.hp <= 0:
        return None, 0.0, rounds
    if b.hp <= 0:
        return 0, a.hp / a.maxhp, rounds
    if a.hp <= 0:
        return 1, b.hp / b.maxhp, rounds
    fa, fb = a.hp / a.maxhp, b.hp / b.maxhp  # timeout: higher HP% wins
    return (0, fa, rounds) if fa >= fb else (1, fb, rounds)


# --------------------------------------------------------------------------
# Damage-per-turn analysis
# --------------------------------------------------------------------------
def dpt_series(name, turns):
    f, dummy = make(name), make_dummy()
    dmg, hits, stuns = [], [], []
    for _ in range(turns):
        d, h, s = take_turn(f, dummy)
        take_turn(dummy, f)  # lets the dummy burn off any stun
        dmg.append(d)
        hits.append(h)
        stuns.append(s)
    return np.array(dmg), np.array(hits), np.array(stuns)


def build_dataset(turns):
    data = {n: dict(zip(("dmg", "hits", "stuns"), dpt_series(n, turns)))
            for n in ALL_NAMES}
    ref_dpt = float(np.mean([data[n]["dmg"].mean() for n in BASE_ROSTER]))
    ref_hits = float(np.mean([data[n]["hits"].mean() for n in BASE_ROSTER]))
    for n in ALL_NAMES:
        f = make(n)
        refl = sum(b.retaliate * b.focus for b in f.bodies)
        d = data[n]
        d["reflect"] = np.full(turns, refl * ref_hits)
        d["stun_credit"] = d["stuns"] * ref_dpt
        d["adjusted"] = d["dmg"] + d["reflect"] + d["stun_credit"]
    return data, ref_dpt, ref_hits


def plot_damage_per_turn(data, turns, ref_dpt, ref_hits, path):
    n = len(ALL_NAMES)
    cols = 4
    rows = int(np.ceil(n / cols))
    fig, axes = plt.subplots(rows, cols, figsize=(5 * cols, 3.1 * rows),
                             sharex=True, sharey=True)
    axes = axes.ravel()
    x = np.arange(1, turns + 1)
    ymax = max(d["adjusted"].max() for d in data.values()) * 1.1
    for ax, name in zip(axes, ALL_NAMES):
        d = data[name]
        ax.bar(x, d["dmg"], color="#3b82f6", label="Raw damage")
        ax.bar(x, d["reflect"], bottom=d["dmg"], color="#f59e0b",
               label="Reflect (Ren)")
        ax.bar(x, d["stun_credit"], bottom=d["dmg"] + d["reflect"],
               color="#22c55e", hatch="//", edgecolor="white",
               label="Stun credit (-1 enemy turn)")
        ax.set_title(f"{name}\navg {d['dmg'].mean():.0f} raw -> "
                     f"{d['adjusted'].mean():.0f} adjusted / turn", fontsize=9)
        ax.set_ylim(0, ymax)
        ax.grid(axis="y", alpha=0.3)
    for ax in axes[n:]:
        ax.axis("off")
    for ax in axes[-cols:]:
        ax.set_xlabel("Turn")
    for ax in axes[::cols]:
        ax.set_ylabel("Damage")
    handles, labels = axes[0].get_legend_handles_labels()
    fig.legend(handles, labels, loc="lower right", bbox_to_anchor=(0.97, 0.04),
               fontsize=10)
    fig.suptitle(f"Damage per turn vs. a training dummy (stun credit = "
                 f"{ref_dpt:.0f} dmg/stun, reflect uses {ref_hits:.2f} hits/turn)",
                 fontsize=13)
    fig.tight_layout(rect=(0, 0, 1, 0.97))
    fig.savefig(path, dpi=130)
    return fig


def plot_summary(data, path):
    order = sorted(ALL_NAMES, key=lambda n: data[n]["adjusted"].mean())
    raw = np.array([data[n]["dmg"].mean() for n in order])
    refl = np.array([data[n]["reflect"].mean() for n in order])
    stun = np.array([data[n]["stun_credit"].mean() for n in order])
    fig, ax = plt.subplots(figsize=(11, 0.45 * len(order) + 1.8))
    y = np.arange(len(order))
    ax.barh(y, raw, color="#3b82f6", label="Raw damage")
    ax.barh(y, refl, left=raw, color="#f59e0b", label="Reflect (Ren)")
    ax.barh(y, stun, left=raw + refl, color="#22c55e", hatch="//",
            edgecolor="white", label="Stun credit")
    for yi, tot, r in zip(y, raw + refl + stun, raw):
        ax.text(tot + 2, yi, f"{tot:.0f}  (raw {r:.0f})", va="center", fontsize=8)
    ax.set_yticks(y)
    ax.set_yticklabels(order)
    ax.set_xlabel("Average damage per turn")
    ax.set_title("Average damage per turn: raw vs. stun/reflect-adjusted")
    ax.legend(loc="lower right")
    ax.grid(axis="x", alpha=0.3)
    ax.set_xlim(0, (raw + refl + stun).max() * 1.2)
    fig.tight_layout()
    fig.savefig(path, dpi=130)
    return fig


# --------------------------------------------------------------------------
# Duel heatmap
# --------------------------------------------------------------------------
def short(name):
    if name == PAIR:
        return "C&I"
    if name.startswith("Prism"):
        return "Pr" + name.split("(")[1].split()[0]
    return {"Ryth": "Ryt", "Vesper Vale": "Ves", "Stellar": "Ste",
            "Vendetta": "Ven", "Marcella": "Mar", "Tavi": "Tav",
            "Willow": "Wil", "Ren": "Ren", "Varg": "Var"}[name]


def plot_heatmap(path):
    names = ALL_NAMES
    n = len(names)
    frac = np.zeros((n, n))
    label = [[""] * n for _ in range(n)]
    for i, a in enumerate(names):
        for j, b in enumerate(names):
            w, f, _ = duel(a, b)
            frac[i, j] = f
            label[i][j] = ("draw" if w is None
                           else f"{short(names[i] if w == 0 else names[j])}\n{f * 100:.0f}%")
    fig, ax = plt.subplots(figsize=(17, 14))
    im = ax.imshow(frac, cmap="RdYlGn", vmin=0, vmax=1)
    for i in range(n):
        for j in range(n):
            ax.text(j, i, label[i][j], ha="center", va="center", fontsize=7)
    ax.set_xticks(range(n))
    ax.set_yticks(range(n))
    ax.set_xticklabels(names, rotation=60, ha="right")
    ax.set_yticklabels(names)
    ax.set_xlabel("Champion B (column)")
    ax.set_ylabel("Champion A (row, acts first on round 1)")
    ax.set_title("1v1 duel: colour = winner's remaining HP "
                 "(red = narrow win, green = dominant win); "
                 "cell text = winner & HP left")
    cb = fig.colorbar(im, ax=ax, fraction=0.035, pad=0.02)
    cb.set_label("Winner's remaining HP (fraction of max)")
    fig.tight_layout()
    fig.savefig(path, dpi=130)
    return frac


# --------------------------------------------------------------------------
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--turns", type=int, default=24,
                    help="turns shown in the damage-per-turn graphs")
    ap.add_argument("--out", default=".", help="output directory")
    ap.add_argument("--show", action="store_true", help="open the figures")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)

    data, ref_dpt, ref_hits = build_dataset(args.turns)
    print(f"{'Champion':<18}{'raw/turn':>10}{'adjusted/turn':>15}")
    for n in sorted(ALL_NAMES, key=lambda k: -data[k]["adjusted"].mean()):
        print(f"{n:<18}{data[n]['dmg'].mean():>10.1f}{data[n]['adjusted'].mean():>15.1f}")

    plot_damage_per_turn(data, args.turns, ref_dpt, ref_hits,
                         os.path.join(args.out, "damage_per_turn.png"))
    plot_summary(data, os.path.join(args.out, "damage_summary.png"))
    frac = plot_heatmap(os.path.join(args.out, "fight_heatmap.png"))
    print("\nSaved damage_per_turn.png, damage_summary.png, fight_heatmap.png")
    if args.show:
        plt.show()


if __name__ == "__main__":
    main()
