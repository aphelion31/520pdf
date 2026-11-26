import { Link } from "react-router";

export default function NotFound() {
  return (
    <div className="mx-auto max-w-xl px-6 py-24 text-center">
      <p className="text-xs font-bold tracking-widest text-[var(--accent)] uppercase">404</p>
      <h1 className="mt-2 text-3xl font-extrabold text-[var(--ink-primary)]">Page not found</h1>
      <p className="mt-2 text-sm text-[var(--ink-secondary)]">
        That route is not part of the 520pdf toolkit.
      </p>
      <Link
        to="/"
        className="mt-6 inline-block rounded-xl bg-[var(--accent)] px-5 py-2.5 text-sm font-semibold text-white hover:bg-[var(--accent-hover)]"
      >
        Back to 520pdf Home
      </Link>
    </div>
  );
}
