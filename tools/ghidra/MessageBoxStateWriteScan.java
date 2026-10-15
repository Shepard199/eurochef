import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.pcode.*;
import ghidra.program.model.symbol.*;
import java.util.*;

public class MessageBoxStateWriteScan extends GhidraScript {
  DecompInterface d=new DecompInterface();
  public void run() throws Exception {
    d.openProgram(currentProgram);
    FunctionIterator it=currentProgram.getFunctionManager().getFunctions(true);
    int hits=0;
    while(it.hasNext()){
      Function f=it.next();
      if(f.getEntryPoint().getOffset()<0x00400000L || f.getEntryPoint().getOffset()>=0x00600000L) continue;
      DecompileResults r=d.decompileFunction(f,20,monitor);
      if(!r.decompileCompleted()||r.getHighFunction()==null) continue;
      Iterator<PcodeOpAST> ops=r.getHighFunction().getPcodeOps();
      boolean printed=false;
      while(ops.hasNext()){
        PcodeOpAST op=ops.next();
        if(op.getOpcode()!=PcodeOp.STORE) continue;
        Varnode[] in=op.getInputs();
        if(in.length<3) continue;
        String s=op.toString();
        if(!s.contains("0xc4") && !s.contains("196")) continue;
        if(!printed){println("\nFUNC "+f.getName()+" @ "+f.getEntryPoint());printed=true;}
        println(op.getSeqnum().getTarget()+" :: "+s);
        hits++;
      }
    }
    println("HITS="+hits);
    d.dispose();
  }
}