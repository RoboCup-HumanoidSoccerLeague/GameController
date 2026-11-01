const Section = ({ title, action, children }) => {
  return (
    <section className="flex w-full flex-col gap-2 rounded-md border border-gray-600 p-3">
      <div className="flex flex-row items-center justify-between gap-2">
        <h2 className="text-xl font-semibold">{title}</h2>
        {action}
      </div>
      {children}
    </section>
  );
};

export default Section;
