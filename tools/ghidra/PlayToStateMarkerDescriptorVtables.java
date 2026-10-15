import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.symbol.Reference;
import java.util.*;

public class PlayToStateMarkerDescriptorVtables extends GhidraScript {
    public void run() throws Exception {
        Memory m=currentProgram.getMemory();
        Set<Long> ctors=new LinkedHashSet<>();
        for(Reference r:getReferencesTo(toAddr(0x005DF8D4L))){
            long d=r.getFromAddress().getOffset();
            if(d<0x005D0000L||d>0x00600000L) continue;
            long ctor=Integer.toUnsignedLong(m.getInt(toAddr(d+12)));
            ctors.add(ctor);
            println(String.format("DESC %08X ctor %08X",d,ctor));
            for(Reference cr:getReferencesTo(toAddr(ctor))){
                long from=cr.getFromAddress().getOffset();
                if(from>=0x005D0000L&&from<=0x00610000L) println(String.format("  DATAREF %08X",from));
            }
        }
    }
}
