import { testimonials as t } from "@/content/testimonials";

// TESTIMONIALS section, transcribed 1:1 from landing/index.html.
export default function Testimonials() {
  return (
    <section className="testi">
      <svg
        className="deco draw sm-hide"
        style={{ left: "6%", top: "34px", width: "54px" }}
        viewBox="0 0 60 44"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M24 6 Q10 10 10 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
        />
        <path
          pathLength="1"
          d="M52 6 Q38 10 38 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
        />
      </svg>
      <svg
        className="deco draw sm-hide"
        style={{
          right: "6%",
          top: "34px",
          width: "54px",
          transform: "rotate(180deg)",
        }}
        viewBox="0 0 60 44"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M24 6 Q10 10 10 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
        />
        <path
          pathLength="1"
          d="M52 6 Q38 10 38 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
        />
      </svg>
      <svg
        className="deco draw xl-only"
        style={{ left: "2%", top: "42%", width: "36px" }}
        viewBox="0 0 40 40"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M20 3 l4.5 11 12 1 -9 8 3 12 -10.5 -7 -10.5 7 3 -12 -9 -8 12 -1 Z"
          style={{ strokeWidth: "2.4" }}
        />
      </svg>
      <svg
        className="deco draw xl-only"
        style={{
          right: "2%",
          top: "60%",
          width: "28px",
          transform: "rotate(-12deg)",
        }}
        viewBox="0 0 40 40"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M20 3 l4.5 11 12 1 -9 8 3 12 -10.5 -7 -10.5 7 3 -12 -9 -8 12 -1 Z"
          style={{ strokeWidth: "2.2" }}
        />
      </svg>
      <svg
        className="deco draw xl-only"
        style={{ left: "3%", bottom: "70px", width: "46px" }}
        viewBox="0 0 50 50"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M8 26 h8 v18 H8 Z"
          style={{ strokeWidth: "2.4" }}
        />
        <path
          pathLength="1"
          d="M16 30 q1 -12 5 -17 q4 -5 6 1 q1 4 -2 9 h10 q5 0 4 5 l-3 12 q-1 4 -6 4 H16"
          style={{ strokeWidth: "2.4" }}
        />
      </svg>
      <h2>
        {t.heading}
        <br />
        <span className="small">{t.headingSmall}</span>
      </h2>
      <div className="wall">
        {t.quotes.map((q, i) => (
          <div className="quote" key={i}>
            <p>{q.quote}</p>
            <span className="who">{q.who}</span>
          </div>
        ))}
        <div className="quote">
          <p className="stars">{t.stars}</p>
          <span className="who">{t.starsWho}</span>
        </div>
      </div>
      <p className="featured">
        {t.featuredPrefix}
        <b>{t.featured[0]}</b>
        {t.sep}
        <b>{t.featured[1]}</b>
        {t.sep}
        <b>{t.featured[2]}</b>
      </p>
    </section>
  );
}
