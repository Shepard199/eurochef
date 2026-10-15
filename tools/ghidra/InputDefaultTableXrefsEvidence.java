import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.*;
public class InputDefaultTableXrefsEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 public void run()throws Exception{d.openProgram(currentProgram);ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(0x005e1758L));while(it.hasNext()){Reference r=it.next();Function f=getFunctionContaining(r.getFromAddress());println(r.getReferenceType()+" "+r.getFromAddress()+" "+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));if(f!=null){DecompileResults dr=d.decompileFunction(f,60,monitor);if(dr.decompileCompleted()&&dr.getDecompiledFunction()!=null)println(dr.getDecompiledFunction().getC());}}d.dispose();}
}