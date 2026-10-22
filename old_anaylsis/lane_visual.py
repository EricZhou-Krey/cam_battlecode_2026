import matplotlib.pyplot as plt
from matplotlib.ticker import MultipleLocator

paths = [[[12, 33], [17, 29], [22, 24], [22, 16], [17, 11], [12, 7]], [], [], [], [], [], [], []]

fig, axes = plt.subplots(2, 4, figsize=(16, 9), sharex=True, sharey=True)
axes = axes.flatten()

for path_number, (ax, path) in enumerate(zip(axes, paths), start=1):
    ax.set_title(f"Path {path_number}")

    if path: 
        xs = [point[0] for point in path]
        ys = [point[1] for point in path]

        ax.plot(xs, ys, color="steelblue", linewidth=1.8, zorder=1)
        ax.scatter(xs, ys, color="steelblue", s=28, zorder=2)

        ax.scatter(*path[0], color="green", s=65, zorder=3, label="Start")
        ax.scatter(*path[-1], color="red", s=65, zorder=3, label="End")

        for step, (x, y) in enumerate(path, start=1):
            ax.annotate(
                str(step), (x, y),
                xytext=(4, 4), textcoords="offset points",
                fontsize=7, color="black"
            )

    ax.set_xlim(0, 40)
    ax.set_ylim(0, 40)
    ax.set_aspect("equal", adjustable="box")
    ax.xaxis.set_major_locator(MultipleLocator(5))
    ax.yaxis.set_major_locator(MultipleLocator(5))
    ax.xaxis.set_minor_locator(MultipleLocator(1))
    ax.yaxis.set_minor_locator(MultipleLocator(1))
    ax.grid(which="major", linewidth=0.8, alpha=0.45)
    ax.grid(which="minor", linewidth=0.4, alpha=0.18)

fig.supxlabel("X")
fig.supylabel("Y")
fig.suptitle("Paths shown separately; point numbers indicate traversal order")
fig.tight_layout()
fig.savefig("lane_paths.png", dpi=300, bbox_inches="tight")
plt.show()

