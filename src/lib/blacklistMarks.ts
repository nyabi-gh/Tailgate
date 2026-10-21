/**
 * The blacklisted posts of one search, as Rust marks them page by page and again when it
 * refreshes the blacklist. A page comes as a command reply and a refresh as an event, which may
 * arrive in either order; the blacklist generation each carries sorts them out.
 */
export class BlacklistMarks {
  #ids: ReadonlySet<number> = new Set();
  /** The blacklist `#ids` was marked with; 0 before the first page. */
  #generation = 0;
  /** Marks from pages of a newer blacklist whose refresh has not arrived yet, by generation. */
  #newer = new Map<number, number[]>();

  get ids(): ReadonlySet<number> {
    return this.#ids;
  }

  /** Returns whether the marks changed. */
  addPage(generation: number, ids: number[]): boolean {
    if (this.#generation === 0) this.#generation = generation;
    // Marked before a refresh that has already marked these posts again.
    if (generation < this.#generation) return false;
    if (generation > this.#generation) this.#newer.set(generation, [...(this.#newer.get(generation) ?? []), ...ids]);
    if (ids.length === 0) return false;
    this.#ids = new Set([...this.#ids, ...ids]);
    return true;
  }

  /** `ids`: the posts loaded before the refresh that the new blacklist hides. Returns whether the marks changed. */
  applyRefresh(generation: number, ids: number[]): boolean {
    if (generation <= this.#generation) return false;
    const later: number[] = [];
    for (const [newer, marked] of this.#newer) {
      if (newer < generation) continue;
      later.push(...marked);
    }
    for (const newer of [...this.#newer.keys()]) if (newer <= generation) this.#newer.delete(newer);
    this.#generation = generation;
    this.#ids = new Set([...ids, ...later]);
    return true;
  }
}
