interface StepDotsProps {
  currentStep: number;
  totalSteps: number;
}

export function StepDots({ currentStep, totalSteps }: StepDotsProps) {
  return (
    <div className="mb-6 mt-6 flex justify-center gap-1.5">
      {Array.from({ length: totalSteps }).map((_, i) => (
        <div
          key={i}
          className={`h-1 rounded-full transition-all duration-300 ${
            i === currentStep ? 'w-5 bg-brand' : 'w-1.5 bg-white/15'
          }`}
        />
      ))}
    </div>
  );
}
