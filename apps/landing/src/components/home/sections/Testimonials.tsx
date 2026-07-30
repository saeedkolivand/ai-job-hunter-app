// Testimonials <section> of / (home), split out of HomeBody.tsx purely for
// file size — verbatim ported markup, no props. See HomeBody.tsx for the
// shared conversion notes; public/scripts/home-0.js binds to .testi and
// .reveal by class (ADR-0018).
export function Testimonials() {
  return (
    <section className="testi">
      <svg
        className="deco draw sm-hide"
        style={{ left: '6%', top: '34px', width: '54px' }}
        viewBox="0 0 60 44"
        aria-hidden="true"
      >
        <path pathLength="1" d="M24 6 Q10 10 10 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10" />
        <path pathLength="1" d="M52 6 Q38 10 38 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10" />
      </svg>
      <svg
        className="deco draw sm-hide"
        style={{ right: '6%', top: '34px', width: '54px', transform: 'rotate(180deg)' }}
        viewBox="0 0 60 44"
        aria-hidden="true"
      >
        <path pathLength="1" d="M24 6 Q10 10 10 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10" />
        <path pathLength="1" d="M52 6 Q38 10 38 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10" />
      </svg>
      <svg
        className="deco draw xl-only"
        style={{ left: '2%', top: '42%', width: '36px' }}
        viewBox="0 0 40 40"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M20 3 l4.5 11 12 1 -9 8 3 12 -10.5 -7 -10.5 7 3 -12 -9 -8 12 -1 Z"
          style={{ strokeWidth: '2.4' }}
        />
      </svg>
      <svg
        className="deco draw xl-only"
        style={{ right: '2%', top: '60%', width: '28px', transform: 'rotate(-12deg)' }}
        viewBox="0 0 40 40"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M20 3 l4.5 11 12 1 -9 8 3 12 -10.5 -7 -10.5 7 3 -12 -9 -8 12 -1 Z"
          style={{ strokeWidth: '2.2' }}
        />
      </svg>
      <svg
        className="deco draw xl-only"
        style={{ left: '3%', bottom: '70px', width: '46px' }}
        viewBox="0 0 50 50"
        aria-hidden="true"
      >
        <path pathLength="1" d="M8 26 h8 v18 H8 Z" style={{ strokeWidth: '2.4' }} />
        <path
          pathLength="1"
          d="M16 30 q1 -12 5 -17 q4 -5 6 1 q1 4 -2 9 h10 q5 0 4 5 l-3 12 q-1 4 -6 4 H16"
          style={{ strokeWidth: '2.4' }}
        />
      </svg>
      <h2>
        what the people are saying
        <br />
        <span className="small">(the people are not real)</span>
      </h2>
      <div className="wall">
        <div className="quote">
          <p>"I applied to 0 jobs and got 0 rejections. 10/10."</p>
          <span className="who">— a guy</span>
        </div>
        <div className="quote">
          <p>"haven't felt my hands in weeks. great app."</p>
          <span className="who">— power user</span>
        </div>
        <div className="quote">
          <p>
            "let it write every application. landed 6 interviews. ghosted all of them. we are the
            same now."
          </p>
          <span className="who">— finally, revenge</span>
        </div>
        <div className="quote">
          <p>"my therapist asked where the rage went. it's in the regex now."</p>
          <span className="who">— anonymous</span>
        </div>
        <div className="quote">
          <p>"downloaded it, my MacBook said the app was 'damaged.' relatable."</p>
          <span className="who">— early adopter</span>
        </div>
        <div className="quote">
          <p>"I don't have a job but I have a workflow."</p>
          <span className="who">— verified ✓ (not verified)</span>
        </div>
        <div className="quote">
          <p>
            "it hits Workday through an aggregator API so I never had to make a Workday account
            again. I wept — the good kind."
          </p>
          <span className="who">— survivor</span>
        </div>
        <div className="quote">
          <p>"5 stars. would dissociate again."</p>
          <span className="who">— a review I'll never get, this isn't on the App Store</span>
        </div>
        <div className="quote">
          <p className="stars">★★★★★</p>
          <span className="who">— my mom (still confused about what I do)</span>
        </div>
      </div>
      <p className="featured">
        as featured in: <b>your group chat</b> · <b>one (1) reddit comment</b> ·{' '}
        <b>my mom's facebook</b>
      </p>
    </section>
  );
}
