// The box of a form control. Controls that aren't boxed natively (e.g. a lone checkbox) use it with
// a transparent border, so they are as tall as and aligned with the neighboring selects. The height
// is fixed because the natural height of a select differs from that of other elements.
export const CONTROL_BOX_CLASSES = "h-7 w-full rounded-md border px-2";

export const selectClasses = (isLegal = true) =>
  `${CONTROL_BOX_CLASSES} bg-white ${isLegal ? "border-gray-600" : "border-red-600"}`;

export const Field = ({ id, label, children }) => {
  return (
    <div className="flex flex-col gap-1">
      <label className="whitespace-nowrap text-sm text-gray-600" htmlFor={id}>
        {label}
      </label>
      {children}
    </div>
  );
};

export const CheckboxField = ({ id, label, checked, onChange, title, type = "checkbox" }) => {
  return (
    <div className="flex flex-row items-center gap-2" title={title}>
      <input
        className="size-4 accent-gray-700"
        type={type}
        checked={checked}
        id={id}
        onChange={onChange}
      />
      <label htmlFor={id}>{label}</label>
    </div>
  );
};

export const Details = ({ summary, children }) => {
  return (
    <details>
      <summary className="cursor-pointer whitespace-nowrap text-sm text-gray-600">
        {summary}
      </summary>
      <div className="mt-2 flex flex-col gap-2 pl-6">{children}</div>
    </details>
  );
};
