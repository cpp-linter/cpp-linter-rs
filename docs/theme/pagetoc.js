let scrollTimeout;

const listenActive = () => {
  const elems = document.querySelector(".pagetoc").children;
  [...elems].forEach((el) => {
    el.addEventListener("click", () => {
      clearTimeout(scrollTimeout);
      [...elems].forEach((el) => el.classList.remove("active"));
      el.classList.add("active");
      // Prevent scroll updates for a short period
      // eslint-disable-next-line no-async-operation
      scrollTimeout = setTimeout(() => {
        scrollTimeout = null;
      }, 100); // Adjust timing as needed
    });
  });
};

const getPagetoc = () => {
  return document.querySelector(".pagetoc") || autoCreatePagetoc();
};

const autoCreatePagetoc = () => {
  const chapter = document.querySelector(
    "body nav#sidebar.sidebar li.chapter-item.expanded a.active"
  );
  const content = Object.assign(document.createElement("div"), {
    className: "content-wrap",
  });
  content.appendChild(chapter.cloneNode(true));
  const divAddedToc = Object.assign(document.createElement("div"), {
    className: "sidetoc",
  });
  const navAddedToc = Object.assign(document.createElement("nav"), {
    className: "pagetoc",
  });
  divAddedToc.appendChild(navAddedToc);
  content.appendChild(divAddedToc);
  chapter.replaceWith(content);
  return document.querySelector(".pagetoc");
};

const updateFunction = () => {
  if (scrollTimeout) return; // Skip updates if within the cooldown period from a click
  const headers = [...document.getElementsByClassName("header")];
  const scrolledY = window.scrollY;

  // Find the last header that is above the current scroll position
  let headerOffsets = headers.filter((el) => scrolledY >= el.offsetTop);
  const lastHeader = headerOffsets.reverse().shift();

  const pagetocLinks = [...document.querySelector(".pagetoc").children];
  pagetocLinks.forEach((link) => link.classList.remove("active"));

  if (lastHeader) {
    const activeLink = pagetocLinks.find(
      (link) => lastHeader.href === link.href
    );
    if (activeLink) activeLink.classList.add("active");
  }
};
function getHeaderText(header) {
  let text = header.textContent;
  if (text === "") {
    let sibling = header.nextSibling;
    let maxIterations = 100;
    while (sibling != null && maxIterations > 0) {
      text += sibling.textContent;
      sibling = sibling.nextSibling;
      maxIterations--;
    }
    if (maxIterations === 0) {
      console.warn(
        "Possible circular reference in DOM when extracting header text"
      );
    }
  }
  return text;
}

const onLoad = () => {
  const pagetoc = getPagetoc();
  var headers = [...document.getElementsByClassName("header")];
  headers.shift();
  headers.forEach((header) => {
    const text = getHeaderText(header);
    const link = Object.assign(document.createElement("a"), {
      textContent: text,
      href: header.href,
      className: `pagetoc-${header.parentElement.tagName}`,
    });
    pagetoc.appendChild(link);
  });
  updateFunction();
  listenActive();
  window.addEventListener("scroll", updateFunction);
};

window.addEventListener("load", onLoad);
