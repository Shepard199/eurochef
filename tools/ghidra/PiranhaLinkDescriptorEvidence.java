import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.listing.Function;

public class PiranhaLinkDescriptorEvidence extends GhidraScript {
    @Override
    public void run() throws Exception {
        Address a = toAddr(0x005EBE08L);
        println("DESCRIPTOR 0x005EBE08");
        for (int off=0; off<0x50; off+=4) {
            long v = Integer.toUnsignedLong(getInt(a.add(off)));
            println(String.format("+0x%02X = 0x%08X", off, v));
        }
        for (Reference ref: getReferencesTo(a)) {
            Function f = getFunctionContaining(ref.getFromAddress());
            println("REF " + ref.getFromAddress() + " " + ref.getReferenceType() + " " +
              (f == null ? "NOFUNC" : f.getName()+"@"+f.getEntryPoint()));
        }
    }
}