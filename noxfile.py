import logging
import sys
import nox

ci_logger = logging.getLogger("CI logger")
ci_handler = logging.StreamHandler(stream=sys.stdout)
ci_handler.formatter = logging.Formatter("%(msg)s")
ci_logger.handlers.append(ci_handler)
ci_logger.propagate = False

nox.options.default_venv_backend = "uv"


def uv_sync(session: nox.Session, *args: str):
    session.run_install(
        "uv",
        "sync",
        "--active",
        *args,
    )


@nox.session(name="docs-rs", python=False)
def docs_rs(session: nox.Session):
    """Build rust API docs"""
    session.run(
        "cargo",
        "doc",
        "--no-deps",
        "--lib",
        "--manifest-path",
        "cpp-linter/Cargo.toml",
        *session.posargs,
        external=True,
    )


def run_mkdocs(session: nox.Session, cmd: str, *args: str):
    """Run mkdocs command"""
    uv_sync(session, "--package", "cli-gen", "--all-groups")
    session.run(
        "uv",
        "run",
        "--package",
        "cli-gen",
        "--active",
        "mkdocs",
        cmd,
        "--config-file",
        "docs/mkdocs.yml",
        *args,
    )


@nox.session(name="docs-build")
def docs_build(session: nox.Session):
    """Build docs with mkdocs"""
    run_mkdocs(session, "build", *session.posargs)


@nox.session(name="docs")
def docs_serve(session: nox.Session):
    """Build docs with mkdocs and serve them.

    To the built docs in your browser::
        uv run nox -s docs -- --open
    """
    run_mkdocs(session, "serve", *session.posargs)


@nox.session(reuse_venv=True)
def test(session: nox.Session):
    """Run unit tests

    To select a profile defined in .config/nextest.toml::
        uv run nox -s test -- --profile ci

    Otherwise, the default profile is used.
    """
    uv_sync(session, "--group", "test")
    session.run(
        "cargo",
        "llvm-cov",
        "--no-report",
        "nextest",
        "--manifest-path",
        "cpp-linter/Cargo.toml",
        "--lib",
        "--tests",
        "--color",
        "always",
        *session.posargs,
        external=True,
    )


@nox.session(name="test-clean", python=False)
def test_clean(session: nox.Session):
    """Purge artifacts from previous test runs.

    This is useful if coverage data needs to be
    completely refreshed.
    """
    session.run("cargo", "llvm-cov", "clean", external=True)


@nox.session(name="llvm-cov", python=False)
def llvm_cov(session: nox.Session):
    """Generate detailed coverage report

    To open the built report in your browser::
        uv run nox -s llvm-cov -- --open
    """
    session.run(
        "cargo",
        "llvm-cov",
        "report",
        "--html",
        "--ignore-filename-regex",
        "main",
        *session.posargs,
        external=True,
    )


@nox.session(name="pretty-cov", python=False)
def pretty_cov(session: nox.Session):
    """Generate pretty coverage report

    To open the built report in your browser::
        uv run nox -s pretty-cov -- --open
    """
    session.run(
        "cargo",
        "llvm-cov",
        "report",
        "--json",
        "--output-path",
        "coverage.json",
        "--ignore-filename-regex",
        "main",
        external=True,
    )
    session.run("llvm-cov-pretty", "coverage.json", external=True, *session.posargs)


@nox.session(python=False)
def lcov(session: nox.Session):
    """Generate lcov.info

    Useful for codecov uploads and VSCode extensions
    like "Coverage Gutters".
    """
    session.run(
        "cargo",
        "llvm-cov",
        "report",
        "--lcov",
        "--output-path",
        "lcov.info",
        "--ignore-filename-regex",
        "main",
        *session.posargs,
        external=True,
    )


@nox.session(python=False)
def lint(session: nox.Session):
    """Run clippy and rustfmt"""
    session.run(
        "cargo", "clippy", "--allow-staged", "--allow-dirty", "--fix", external=True
    )
    session.run("cargo", "fmt", external=True)
